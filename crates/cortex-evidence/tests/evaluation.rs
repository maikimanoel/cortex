//! Guard counterexamples use explicit hypothetical input assertions; no live strategy or independent approval is claimed.
#[path = "support/evaluation.rs"]
mod fixture;
use cortex_core::{binding::*, evaluation::*, program::memory::*};
use cortex_evidence::{ExecutionObservation as Execution, evaluation::*, unified::*};
use fixture::*;
use std::{
    io,
    time::{Duration, UNIX_EPOCH},
};
fn exited(code: u32) -> AttemptExecution {
    AttemptExecution::Observed(Execution::Exited { code })
}

#[test]
fn planned_and_registered_populations_cannot_disappear_from_denominators() {
    let mut ledger = ledger();
    register(&mut ledger, 0, "first", None, Some(0));
    ledger
        .record(observation(
            &ledger,
            0,
            "first",
            exited(0),
            Some(AssessmentResult::Pass),
        ))
        .unwrap();
    let report = inspect(&mut ledger);
    assert_eq!(report.arms.len(), 2);
    assert_eq!(report.arms[0].trials.len(), 1);
    assert_eq!(report.arms[1].trials.len(), 1);
    assert_eq!(report.arms[1].trials[0].attempts, 0);
    assert_eq!(report.arms[0].reported_accepted_trials, 0);
    assert_eq!(report.eligibility, ComparisonEligibility::NotComparable);
    assert!(report.issues.contains(&ComparisonIssue::MissingObservation));
    register(&mut ledger, 1, "open", None, Some(0));
    let report = inspect(&mut ledger);
    assert_eq!(report.arms[1].population.incomplete, 1);
    assert_eq!(report.arms[1].population.registered, 1);
}
#[test]
fn failures_interruption_unknown_skips_blockage_and_na_remain_distinct() {
    for (execution, result, category) in [
        (exited(1), Some(AssessmentResult::Fail), "failed"),
        (
            AttemptExecution::Observed(Execution::Cancelled),
            Some(AssessmentResult::Unknown),
            "interrupted",
        ),
        (
            AttemptExecution::Observed(Execution::TimedOut),
            Some(AssessmentResult::Unknown),
            "interrupted",
        ),
        (AttemptExecution::Unknown, None, "unknown"),
        (
            AttemptExecution::Observed(Execution::NotRun),
            Some(AssessmentResult::NotRun),
            "not_run",
        ),
        (
            AttemptExecution::Observed(Execution::Unavailable),
            Some(AssessmentResult::Blocked),
            "blocked",
        ),
        (
            AttemptExecution::NotApplicable,
            Some(AssessmentResult::NotApplicable),
            "na",
        ),
    ] {
        let mut ledger = ledger();
        let unperformed = matches!(category, "not_run" | "blocked" | "na");
        register(&mut ledger, 0, "attempt", None, (!unperformed).then_some(0));
        ledger
            .record(observation(&ledger, 0, "attempt", execution, result))
            .unwrap();
        let report = inspect(&mut ledger);
        let p = &report.arms[0].population;
        assert_eq!(p.registered, 1);
        assert_eq!(p.incomplete, 0);
        assert_eq!(report.arms[0].reported_accepted_trials, 0);
        assert!(!report.primary_outcome_available);
        assert_eq!(
            match category {
                "failed" => p.failed,
                "interrupted" => p.interrupted,
                "unknown" => p.unknown,
                "not_run" => p.not_run,
                "blocked" => p.blocked,
                _ => p.not_applicable,
            },
            1
        );
        assert_eq!(p.completed, usize::from(category == "failed"));
    }
}
#[test]
fn successful_retry_preserves_failure_costs_and_counts_one_work_item() {
    let mut ledger = ledger();
    register(&mut ledger, 0, "failed", None, Some(0));
    let mut failed = observation(
        &ledger,
        0,
        "failed",
        exited(1),
        Some(AssessmentResult::Fail),
    );
    failed.metrics = MetricObservations::new(
        &[(Metric::CpuTimeNanos, Some(7)), (Metric::InputTokens, None)],
        None,
    )
    .unwrap();
    ledger.record(failed.clone()).unwrap();
    register(&mut ledger, 0, "repair", Some("failed"), Some(90));
    let mut repair = observation(
        &ledger,
        0,
        "repair",
        exited(0),
        Some(AssessmentResult::Pass),
    );
    repair.interval = Some(ElapsedInterval::new(90, 100, TimingBasis::Estimate).unwrap());
    repair.metrics = MetricObservations::new(
        &[
            (Metric::CpuTimeNanos, Some(3)),
            (Metric::InputTokens, Some(11)),
        ],
        None,
    )
    .unwrap();
    repair.endpoint.as_mut().unwrap().acceptance = Some(AcceptanceClaim {
        assessor: reference("reviewer"),
        authority: reference("synthetic-claim"),
        separation: ReviewerSeparation::SeparateAuthorshipSharedContext,
        scope: ResultScope::Integrated,
        accepted_elapsed_ns: Some(120),
    });
    ledger.record(repair.clone()).unwrap();
    let report = inspect(&mut ledger);
    let a = &report.arms[0];
    assert_eq!(a.trials[0].attempts, 2);
    assert_eq!(a.trials[0].retries, 1);
    assert_eq!(a.population.failed, 1);
    assert_eq!(a.population.completed, 2);
    assert_eq!(a.reported_accepted_trials, 1);
    assert_eq!(a.metrics[0].known_subtotal, 10);
    assert_eq!(a.metrics[0].total, Some(10));
    assert_eq!(a.metrics[3].known_subtotal, 11);
    assert_eq!(a.metrics[3].missing_attempts, 1);
    assert_eq!(a.metrics[3].total, None);
    assert_eq!(a.trials[0].reported_primary_elapsed_ns, None);
    assert_eq!(report.scope, MeasurementScope::Synthetic);
    assert!(!report.primary_outcome_available);
    assert_eq!(ledger.attempts()[0].observation(), Some(&failed));
    assert!(matches!(
        ledger.record(repair),
        Err(EvaluationInputError::Terminal)
    ));
    let repeated = ledger.attempts()[0].definition().clone();
    assert!(matches!(
        ledger.register_attempt(repeated),
        Err(EvaluationInputError::Identity)
    ));
}
#[test]
fn missing_metrics_are_not_zero_and_memory_samples_are_not_allocations() {
    let mut ledger = ledger();
    register(&mut ledger, 0, "a", None, Some(0));
    let mut a = observation(&ledger, 0, "a", exited(0), Some(AssessmentResult::Pass));
    let sample = MemoryObservation::new(
        MemoryCorrespondence::ScopeUnestablished(MemoryBasis::ProcessResident),
        None,
    );
    a.metrics =
        MetricObservations::new(&[(Metric::ModelInvocations, Some(0))], Some(sample.clone()))
            .unwrap();
    ledger.record(a).unwrap();
    let report = inspect(&mut ledger);
    assert_eq!(report.arms[0].metrics[2].total, Some(0));
    assert_eq!(report.arms[0].metrics[0].total, None);
    assert_eq!(sample.bytes(), None);
    assert_eq!(
        ledger.attempts()[0].observation().unwrap().metrics.memory(),
        Some(&sample)
    );
    assert_eq!(report.arms[1].metrics[2].total, None);
}
#[test]
fn exact_observation_correspondence_and_terminal_history_are_guarded() {
    let mut ledger = ledger();
    register(&mut ledger, 0, "a", None, Some(0));
    let original = observation(&ledger, 0, "a", exited(0), Some(AssessmentResult::Pass));
    let mut variants = vec![];
    let mut bad = original.clone();
    bad.attempt = AttemptId::new("absent").unwrap();
    variants.push(bad);
    let mut bad = original.clone();
    bad.context.case_revision = rev(2);
    variants.push(bad);
    let mut bad = original.clone();
    bad.context.oracle = reference("wrong-oracle");
    variants.push(bad);
    let mut bad = original.clone();
    bad.context.specification = reference("wrong-spec");
    variants.push(bad);
    let mut bad = original.clone();
    bad.context.configuration.environment = reference("other-host");
    variants.push(bad);
    let mut bad = original.clone();
    bad.context.source = SourceIdentity::new(
        CandidateRequirement::new(
            source().candidate().repository().clone(),
            candidate("c"),
            WorktreeRequirement::Clean,
        ),
        SourceTreeId::new(&"d".repeat(40)).unwrap(),
    )
    .unwrap();
    variants.push(bad);
    for bad in variants {
        assert!(ledger.record(bad).is_err());
        assert!(ledger.attempts()[0].observation().is_none());
        assert_eq!(inspect(&mut ledger).arms[0].population.incomplete, 1);
    }
    ledger.record(original.clone()).unwrap();
    assert!(ledger.record(original).is_err());
    assert_eq!(ledger.attempts().len(), 1);
}
#[test]
fn changed_oracle_source_specification_and_full_profile_stay_invalidated() {
    for change in ["oracle", "source", "task", "profile"] {
        let mut ledger = ledger();
        let spec = ledger.specification().clone();
        let mut cases = spec.cases().to_vec();
        let mut mapping = ledger.oracles().to_vec();
        match change {
            "oracle" => {
                cases[0].oracle = reference("oracle-v2");
                mapping[0].oracle = cases[0].oracle.clone();
            }
            "source" => {
                let candidate = CandidateRequirement::new(
                    source().candidate().repository().clone(),
                    candidate("c"),
                    WorktreeRequirement::Clean,
                );
                cases[0].source = SourceIdentity::new(
                    candidate.clone(),
                    SourceTreeId::new(&"d".repeat(40)).unwrap(),
                )
                .unwrap();
                cases[0].contract = CandidateBindingContract::new(
                    cases[0].contract.task().clone(),
                    rev(1),
                    candidate,
                );
            }
            "task" => {
                cases[0].contract = CandidateBindingContract::new(
                    cortex_core::TaskContract::new(
                        cases[0].contract.task().id().clone(),
                        "Changed objective",
                        "Changed acceptance",
                    )
                    .unwrap(),
                    rev(2),
                    cases[0].source.candidate().clone(),
                );
            }
            _ => {
                mapping[0].profile = VerificationProfile::new(
                    mapping[0].profile.provenance().clone(),
                    "v2",
                    "changed environment",
                    &[],
                )
                .unwrap()
            }
        }
        let changed = replace(&spec, cases, spec.arms().to_vec(), spec.trials().to_vec()).unwrap();
        let report = ledger.inspect(&changed, &mapping).unwrap();
        assert!(
            report
                .issues
                .contains(&ComparisonIssue::SpecificationChanged)
        );
        assert!(
            inspect(&mut ledger)
                .issues
                .contains(&ComparisonIssue::SpecificationChanged)
        );
    }
}
#[test]
fn matched_controls_are_structurally_comparable_but_do_not_establish_an_endpoint() {
    let mut ledger = ledger();
    for i in 0..2 {
        let id = format!("attempt-{i}");
        register(&mut ledger, i, &id, None, Some(0));
        ledger
            .record(observation(
                &ledger,
                i,
                &id,
                exited(0),
                Some(AssessmentResult::Pass),
            ))
            .unwrap();
    }
    let report = inspect(&mut ledger);
    assert_eq!(
        report.eligibility,
        ComparisonEligibility::ComparableWithinDeclaredControls
    );
    assert_eq!(report.scope, MeasurementScope::Synthetic);
    assert!(!report.primary_outcome_available);
    assert!(
        report
            .issues
            .contains(&ComparisonIssue::MissingAcceptedEndpoint)
    );
    let spec = specification();
    for change in [
        "model",
        "tools",
        "environment",
        "information",
        "resources",
        "unknown",
    ] {
        let mut arms = spec.arms().to_vec();
        let c = &mut arms[1].configuration;
        match change {
            "model" => c.model = ModelConfiguration::Configured(reference("different-model")),
            "tools" => c.tools = vec![reference("different-tool")],
            "environment" => c.environment = reference("different-host"),
            "information" => c.information = reference("more-information"),
            "unknown" => c.envelope[0].amount = None,
            _ => c.envelope[0].amount = Some(3),
        };
        let changed = replace(&spec, spec.cases().to_vec(), arms, spec.trials().to_vec()).unwrap();
        let mut ledger = EvaluationLedger::new(changed.clone(), oracles(&changed)).unwrap();
        assert!(
            inspect(&mut ledger)
                .issues
                .contains(&ComparisonIssue::ConfoundedControls)
        );
    }
}
#[test]
fn omitted_repetitions_and_unavailable_live_arms_are_not_comparable() {
    let spec = specification();
    let mut trials = spec.trials().to_vec();
    trials[1].repetition = 2;
    let changed = replace(&spec, spec.cases().to_vec(), spec.arms().to_vec(), trials).unwrap();
    let mut ledger = EvaluationLedger::new(changed.clone(), oracles(&changed)).unwrap();
    assert!(
        inspect(&mut ledger)
            .issues
            .contains(&ComparisonIssue::PopulationMismatch)
    );
    let mut arms = spec.arms().to_vec();
    for a in &mut arms {
        a.configuration.profile = ExecutionProfile::LiveProvider;
        a.configuration.model = ModelConfiguration::Configured(reference("planned-model"));
    }
    let live = replace(&spec, spec.cases().to_vec(), arms, spec.trials().to_vec()).unwrap();
    let mut ledger = EvaluationLedger::new(live.clone(), oracles(&live)).unwrap();
    register(&mut ledger, 0, "blocked", None, None);
    let blocked = observation(
        &ledger,
        0,
        "blocked",
        AttemptExecution::Observed(Execution::Unavailable),
        Some(AssessmentResult::Blocked),
    );
    ledger.record(blocked).unwrap();
    register(&mut ledger, 1, "forged", None, Some(0));
    let mut forged = observation(
        &ledger,
        1,
        "forged",
        exited(0),
        Some(AssessmentResult::Pass),
    );
    forged.origin = ObservationOrigin::DevelopmentFixture;
    assert!(matches!(
        ledger.record(forged),
        Err(EvaluationInputError::ExecutionProfile)
    ));
    assert_eq!(inspect(&mut ledger).arms[0].population.blocked, 1);
}
#[test]
fn synthetic_measurements_estimates_and_bad_timestamps_cannot_be_promoted() {
    let mut ledger = ledger();
    register(&mut ledger, 0, "a", None, Some(0));
    let original = observation(&ledger, 0, "a", exited(0), Some(AssessmentResult::Pass));
    let mut bad = original.clone();
    bad.interval = Some(ElapsedInterval::new(0, 80, TimingBasis::MonotonicObservation).unwrap());
    assert!(matches!(
        ledger.record(bad),
        Err(EvaluationInputError::Timing)
    ));
    let mut bad = original.clone();
    bad.recorded_at = UNIX_EPOCH + Duration::from_secs(2);
    assert!(matches!(
        ledger.record(bad),
        Err(EvaluationInputError::Timing)
    ));
    let mut bad = original.clone();
    bad.phases = vec![PhaseObservation {
        phase: EngineeringPhase::Review,
        interval: ElapsedInterval::new(80, 81, TimingBasis::Estimate).unwrap(),
    }];
    assert!(matches!(
        ledger.record(bad),
        Err(EvaluationInputError::Timing)
    ));
    let mut bad = original.clone();
    bad.release.as_mut().unwrap().clock = reference("restarted-clock");
    assert!(matches!(
        ledger.record(bad),
        Err(EvaluationInputError::Timing)
    ));
    assert!(ElapsedInterval::new(2, 1, TimingBasis::MonotonicObservation).is_err());
    ledger.record(original).unwrap();
}
#[test]
fn retry_cannot_precede_any_known_start_even_when_previous_end_is_missing() {
    let mut ledger = ledger();
    register(&mut ledger, 0, "a", None, Some(100));
    let mut a = observation(&ledger, 0, "a", exited(1), Some(AssessmentResult::Fail));
    a.interval = None;
    a.release = None;
    ledger.record(a).unwrap();
    let trial = ledger.specification().trials()[0].id.clone();
    let bad = AttemptDefinition {
        id: AttemptId::new("retry").unwrap(),
        trial: trial.clone(),
        predecessor: Some(AttemptId::new("a").unwrap()),
        executor: reference("executor"),
        start_ns: Some(0),
    };
    assert!(matches!(
        ledger.register_attempt(bad),
        Err(EvaluationInputError::Timing)
    ));
    register(&mut ledger, 0, "retry", Some("a"), Some(100));
    assert_eq!(ledger.attempts().len(), 2);
    let fork = AttemptDefinition {
        id: AttemptId::new("fork").unwrap(),
        trial,
        predecessor: Some(AttemptId::new("a").unwrap()),
        executor: reference("executor"),
        start_ns: Some(200),
    };
    assert!(ledger.register_attempt(fork).is_err());
}
#[test]
fn same_author_local_or_inconsistent_evidence_cannot_establish_primary_acceptance() {
    for kind in [
        "same-author",
        "prior-executor",
        "local",
        "wrong-source",
        "wrong-profile",
        "nonpass",
    ] {
        let mut ledger = ledger();
        register(&mut ledger, 0, "a", None, Some(0));
        let mut observation = observation(&ledger, 0, "a", exited(0), Some(AssessmentResult::Pass));
        let mut endpoint = endpoint(
            ledger.specification(),
            "a",
            &exited(0),
            AssessmentResult::Pass,
            true,
        );
        match kind {
            "same-author" => {
                endpoint.acceptance.as_mut().unwrap().separation = ReviewerSeparation::SameAuthor
            }
            "prior-executor" => {
                endpoint.acceptance.as_mut().unwrap().assessor = reference("evaluated-executor")
            }
            "local" => endpoint.acceptance.as_mut().unwrap().scope = ResultScope::Local,
            "wrong-source" => {
                endpoint.source = SourceIdentity::new(
                    CandidateRequirement::new(
                        source().candidate().repository().clone(),
                        candidate("c"),
                        WorktreeRequirement::Clean,
                    ),
                    SourceTreeId::new(&"d".repeat(40)).unwrap(),
                )
                .unwrap()
            }
            "wrong-profile" => {
                endpoint.candidate = capture(
                    &CandidateBindingContract::new(
                        endpoint.candidate.contract().task().clone(),
                        rev(2),
                        source().candidate().clone(),
                    ),
                    "wrong-profile",
                    2,
                )
            }
            _ => {
                endpoint = fixture::endpoint(
                    ledger.specification(),
                    "a",
                    &exited(0),
                    AssessmentResult::Fail,
                    false,
                );
                endpoint.acceptance = Some(AcceptanceClaim {
                    assessor: reference("assessor"),
                    authority: reference("claim"),
                    scope: ResultScope::Integrated,
                    separation: ReviewerSeparation::SeparateAuthorshipSharedContext,
                    accepted_elapsed_ns: Some(100),
                });
            }
        }
        observation.endpoint = Some(endpoint);
        let result = ledger.record(observation);
        if matches!(kind, "wrong-source" | "wrong-profile" | "nonpass") {
            assert!(result.is_err());
        } else {
            result.unwrap();
            assert_eq!(inspect(&mut ledger).arms[0].reported_accepted_trials, 0);
        }
    }
}
#[test]
fn duplicate_metrics_overflow_and_report_io_failure_are_visible() {
    assert!(
        MetricObservations::new(
            &[
                (Metric::InputTokens, Some(0)),
                (Metric::InputTokens, Some(1))
            ],
            None
        )
        .is_err()
    );
    let mut ledger = ledger();
    register(&mut ledger, 0, "a", None, Some(0));
    let mut a = observation(&ledger, 0, "a", exited(1), Some(AssessmentResult::Fail));
    a.metrics = MetricObservations::new(&[(Metric::InputTokens, Some(u64::MAX))], None).unwrap();
    ledger.record(a).unwrap();
    register(&mut ledger, 0, "b", Some("a"), Some(100));
    let mut b = observation(&ledger, 0, "b", exited(0), Some(AssessmentResult::Pass));
    b.interval = Some(ElapsedInterval::new(100, 120, TimingBasis::Estimate).unwrap());
    b.metrics = MetricObservations::new(&[(Metric::InputTokens, Some(1))], None).unwrap();
    assert!(matches!(
        ledger.record(b),
        Err(EvaluationInputError::Overflow)
    ));
    assert!(ledger.attempts()[1].observation().is_none());
    let report = inspect(&mut ledger);
    let mut text = Vec::new();
    report.write_redacted(&mut text).unwrap();
    let text = String::from_utf8(text).unwrap();
    assert!(text.contains("improvement claim: NOT_ESTABLISHED"));
    assert!(!text.contains("evaluation-fixture-owner") && !text.contains("/synthetic"));
    struct Broken;
    impl io::Write for Broken {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::ErrorKind::BrokenPipe.into())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    assert!(report.write_redacted(&mut Broken).is_err());
}

// These are hypothetical reported fixture/acceptance inputs for consistency
// testing, not real independent acceptance or observed strategy performance.
fn reported_fixture_ledger() -> EvaluationLedger {
    let spec = specification();
    let mut arms = spec.arms().to_vec();
    for a in &mut arms {
        a.configuration.profile = ExecutionProfile::ProviderFreeFixture;
        a.configuration.model = ModelConfiguration::NotApplicable;
    }
    let spec = replace(&spec, spec.cases().to_vec(), arms, spec.trials().to_vec()).unwrap();
    EvaluationLedger::new(spec.clone(), oracles(&spec)).unwrap()
}
fn reported_fixture_observation(
    ledger: &EvaluationLedger,
    index: usize,
    id: &str,
    start: u64,
    end: u64,
    accepted: u64,
) -> AttemptObservation {
    let mut o = observation(ledger, index, id, exited(0), Some(AssessmentResult::Pass));
    o.origin = ObservationOrigin::DevelopmentFixture;
    o.interval = Some(ElapsedInterval::new(start, end, TimingBasis::MonotonicObservation).unwrap());
    o.endpoint.as_mut().unwrap().acceptance = Some(AcceptanceClaim {
        assessor: reference("separate-assessor"),
        authority: reference("hypothetical-fixture-acceptance-input"),
        separation: ReviewerSeparation::SeparateAuthorshipSharedContext,
        scope: ResultScope::Integrated,
        accepted_elapsed_ns: Some(accepted),
    });
    o
}
#[test]
fn reported_primary_endpoint_uses_explicit_acceptance_offset_across_retry_gap() {
    let mut ledger = reported_fixture_ledger();
    register(&mut ledger, 0, "failure", None, Some(0));
    let mut failed = observation(
        &ledger,
        0,
        "failure",
        exited(1),
        Some(AssessmentResult::Fail),
    );
    failed.origin = ObservationOrigin::DevelopmentFixture;
    failed.interval = Some(ElapsedInterval::new(0, 40, TimingBasis::MonotonicObservation).unwrap());
    ledger.record(failed).unwrap();
    register(&mut ledger, 0, "repair", Some("failure"), Some(90));
    let repaired = reported_fixture_observation(&ledger, 0, "repair", 90, 100, 150);
    ledger.record(repaired).unwrap();
    register(&mut ledger, 1, "neighbor", None, Some(0));
    let neighbor = reported_fixture_observation(&ledger, 1, "neighbor", 0, 70, 130);
    ledger.record(neighbor).unwrap();
    let report = inspect(&mut ledger);
    assert_eq!(
        report.eligibility,
        ComparisonEligibility::ComparableWithinDeclaredControls
    );
    assert!(report.primary_outcome_available);
    assert_eq!(
        report.arms[0].trials[0].reported_primary_elapsed_ns,
        Some(150)
    );
    assert_eq!(
        report.arms[1].trials[0].reported_primary_elapsed_ns,
        Some(130)
    );
    assert_eq!(report.arms[0].population.failed, 1);
    assert_eq!(report.arms[0].reported_accepted_trials, 1);
    assert_eq!(report.arms[0].trials[0].attempts, 2);
    assert_eq!(report.arms[0].metrics[0].total, None); // No resource-efficiency claim from time.
}
#[test]
fn accepted_event_and_executor_chain_cannot_be_time_reversed_or_self_approved() {
    let mut ledger = reported_fixture_ledger();
    register(&mut ledger, 0, "first", None, Some(0));
    let bad = reported_fixture_observation(&ledger, 0, "first", 0, 80, 79);
    assert!(matches!(
        ledger.record(bad),
        Err(EvaluationInputError::Timing)
    ));
    let first = reported_fixture_observation(&ledger, 0, "first", 0, 80, 120);
    ledger.record(first).unwrap();
    let trial = ledger.specification().trials()[0].id.clone();
    let reversed = AttemptDefinition {
        id: AttemptId::new("reversed").unwrap(),
        trial: trial.clone(),
        predecessor: Some(AttemptId::new("first").unwrap()),
        executor: reference("second-executor"),
        start_ns: Some(90),
    };
    assert!(matches!(
        ledger.register_attempt(reversed),
        Err(EvaluationInputError::Timing)
    ));
    ledger
        .register_attempt(AttemptDefinition {
            id: AttemptId::new("second").unwrap(),
            trial,
            predecessor: Some(AttemptId::new("first").unwrap()),
            executor: reference("second-executor"),
            start_ns: Some(121),
        })
        .unwrap();
    let mut second = reported_fixture_observation(&ledger, 0, "second", 121, 130, 150);
    second
        .endpoint
        .as_mut()
        .unwrap()
        .acceptance
        .as_mut()
        .unwrap()
        .assessor = reference("evaluated-executor"); // First executor, not the last.
    ledger.record(second).unwrap();
    let report = inspect(&mut ledger);
    assert_eq!(report.arms[0].reported_accepted_trials, 0);
    assert_eq!(report.arms[0].trials[0].reported_primary_elapsed_ns, None);
}
#[test]
fn malformed_current_oracle_pool_does_not_partially_invalidate_original_context() {
    let mut ledger = ledger();
    let spec = ledger.specification().clone();
    let mut cases = spec.cases().to_vec();
    cases[0].oracle = reference("changed-oracle");
    let changed = replace(&spec, cases, spec.arms().to_vec(), spec.trials().to_vec()).unwrap();
    assert!(ledger.inspect(&changed, &[]).is_err());
    assert!(
        !inspect(&mut ledger)
            .issues
            .contains(&ComparisonIssue::SpecificationChanged)
    );
    assert!(ledger.first_invalidation().is_none());
}

#[test]
fn unavailable_or_model_free_profiles_cannot_launder_provider_consumption() {
    let mut ledger = reported_fixture_ledger();
    register(&mut ledger, 0, "fixture", None, Some(0));
    let mut o = reported_fixture_observation(&ledger, 0, "fixture", 0, 80, 100);
    o.metrics = MetricObservations::new(&[(Metric::ModelInvocations, Some(1))], None).unwrap();
    assert!(matches!(
        ledger.record(o),
        Err(EvaluationInputError::ExecutionProfile)
    ));
    assert_eq!(inspect(&mut ledger).arms[0].population.incomplete, 1);
}

#[test]
fn unknown_predecessor_and_incomplete_metrics_survive_a_reported_successful_retry() {
    let mut ledger = reported_fixture_ledger();
    register(&mut ledger, 0, "unknown", None, Some(0));
    let mut unknown = observation(&ledger, 0, "unknown", AttemptExecution::Unknown, None);
    unknown.origin = ObservationOrigin::DevelopmentFixture;
    unknown.interval =
        Some(ElapsedInterval::new(0, 40, TimingBasis::MonotonicObservation).unwrap());
    ledger.record(unknown).unwrap();
    register(&mut ledger, 0, "retry", Some("unknown"), Some(50));
    let o = reported_fixture_observation(&ledger, 0, "retry", 50, 70, 100);
    ledger.record(o).unwrap();
    let report = inspect(&mut ledger);
    assert_eq!(report.arms[0].population.unknown, 1);
    assert_eq!(report.arms[0].population.completed, 1);
    assert_eq!(report.arms[0].reported_accepted_trials, 0);
    assert_eq!(report.arms[0].trials[0].reported_primary_elapsed_ns, None);
    assert_eq!(report.arms[0].metrics[0].total, None);
    assert!(report.issues.contains(&ComparisonIssue::UnknownOutcome));
}
