#![cfg(target_os = "linux")]
//! Pure cross-package composition with a new encrypted journal; no child launches.
#[path = "support/program_composition.rs"]
mod composition;
#[path = "../../cortex-evidence/tests/support/program_evaluation.rs"]
mod model;
use cortex_cli::program_preparation::{TaskOperations, prepare};
use cortex_core::program::{context::*, operational::*, scheduling::Proposal, *};
use cortex_evidence::{
    evaluation::{program::*, *},
    program::{Command, CoordinationKernel, Operation},
};
use cortex_storage::{
    history_store::{Access, StoreKey},
    program_store::{ProgramStore, ReadLimits},
};
use model::*;
use std::{fs, path::PathBuf, time::Duration};
fn pair(kernel: &CoordinationKernel, name: &str) -> [Operation; 2] {
    let scope = kernel.state().readiness().scope(&task(name)).unwrap();
    let attempt = Attempt {
        id: cortex_core::program::AttemptId::new(&format!("joined-attempt-{name}")).unwrap(),
        increment: IncrementId::new(name).unwrap(),
        owner: ActorId::new("operator").unwrap(),
        baseline: rev(1),
        generation: scope.generation(),
        status: AttemptStatus::Claimed,
        basis: source("explicit-test-claim"),
    };
    let request = PackageRequest {
        id: PackageId::new(&format!("joined-package-{name}")).unwrap(),
        revision: rev(1),
        task: task(name),
        baseline: rev(1),
        generation: scope.generation(),
        sources: vec![],
        methods: vec![],
        omissions: "Pure contract fixture".into(),
        deliverable: "Complete declared population".into(),
        stop_rules: "No physical executor or automatic retry".into(),
    };
    let profile = oracles(&specification(kernel))[0].profile.clone();
    let request = composition::advisory_package(
        kernel.state(),
        request,
        &capture(scope.context().contract(), 2),
        &profile,
    );
    let first = operation(
        kernel,
        &format!("joined-claim-{name}"),
        Command::Claim(attempt),
    );
    let mut second = operation(
        kernel,
        &format!("joined-package-{name}"),
        Command::Package(request),
    );
    second.expected = first.expected + 1;
    [first, second]
}
fn proposal(evaluation: &ProgramEvaluation, name: &str) -> Proposal {
    Proposal {
        task: task(name),
        band: 0,
        waiting_rounds: 0,
        risk_rank: 0,
        cost_rank: 0,
        demand: evaluation.ledger().specification().arms()[0]
            .configuration
            .envelope
            .clone(),
    }
}
fn prepare_task(
    kernel: &CoordinationKernel,
    evaluation: &ProgramEvaluation,
    name: &str,
) -> Vec<Operation> {
    let scope = kernel.state().readiness().scope(&task(name)).unwrap();
    prepare(
        kernel,
        kernel.revision(),
        &[proposal(evaluation, name)],
        &[TaskOperations {
            task: task(name),
            candidate: scope.context().contract().candidate().clone(),
            operations: Some(pair(kernel, name)),
        }],
    )
    .unwrap()
    .into_operations()
}
fn prepare_attempt(
    kernel: &mut CoordinationKernel,
    evaluation: &ProgramEvaluation,
    name: &str,
) -> ProgramAttempt {
    let prepared = prepare_task(kernel, evaluation, name);
    assert_eq!(prepared.len(), 2);
    let claim = prepared[0].id.clone();
    let package = prepared[1].id.clone();
    for input in prepared {
        kernel.admit(input).unwrap();
    }
    let scope = kernel.state().readiness().scope(&task(name)).unwrap();
    let effect_scope = EffectScope {
        program: kernel.state().definition().id().clone(),
        baseline: rev(1),
        task: task(name),
        candidate: scope.context().contract().candidate().clone(),
        attempt: cortex_core::program::AttemptId::new(&format!("joined-attempt-{name}")).unwrap(),
        generation: scope.generation(),
        package: PackageId::new(&format!("joined-package-{name}")).unwrap(),
        package_revision: rev(1),
    };
    reserve_existing(kernel, name, effect_scope, claim, package, None)
}
fn bytes(root: &std::path::Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut entries = fs::read_dir(root)
        .unwrap()
        .map(|e| {
            let p = e.unwrap().path();
            let b = fs::read(&p).unwrap();
            (p, b)
        })
        .collect::<Vec<_>>();
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    entries
}
#[test]
fn method_preparation_and_complete_evaluation_preserve_authoritative_journal() {
    let mut kernel = initial_kernel();
    let mut evaluation = declared(&kernel);
    assert!(prepare_task(&kernel, &evaluation, "B").is_empty());
    let a = join(&mut kernel, &mut evaluation, "A");
    report(&mut kernel, &a, EffectOutcome::Succeeded);
    let obs = observation(&evaluation, &a, exited(0));
    evaluation.record(&kernel, obs).unwrap();
    release(&mut kernel, "AB");
    let b = join(&mut kernel, &mut evaluation, "B");
    report(&mut kernel, &b, EffectOutcome::Succeeded);
    let obs = observation(&evaluation, &b, exited(0));
    evaluation.record(&kernel, obs).unwrap();
    let root = std::env::temp_dir().join(format!("cortex-composition-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    let key = StoreKey::new(&[0xb7; 32]).unwrap();
    let store = ProgramStore::create_fixture(
        &root.join("store"),
        &key,
        kernel.initial().clone(),
        ReadLimits::new(16_000_000, Duration::from_secs(20)).unwrap(),
    )
    .unwrap();
    for input in kernel.operations() {
        store.admit(&key, input.clone()).unwrap();
    }
    let before = bytes(&root.join("store"));
    let revision = kernel.revision();
    let result = inspect(&mut evaluation, &kernel);
    assert_eq!(result.evaluation.arms[0].population.registered, 2);
    assert_eq!(result.evaluation.arms[0].population.completed, 2);
    assert_eq!(result.evaluation.arms[0].trials.len(), 6);
    assert_eq!(result.evaluation.arms[1].trials.len(), 6);
    assert_eq!(result.evaluation.arms[1].population.registered, 0);
    assert!(!result.evaluation.primary_outcome_available);
    inspect(&mut evaluation, &kernel);
    assert_eq!(bytes(&root.join("store")), before);
    assert_eq!(
        store.load(&key, Access::ReadOnly).unwrap().revision(),
        revision
    );
    assert!(
        kernel
            .packages()
            .iter()
            .all(|p| p.request().methods.len() == 1)
    );
    eprintln!("Retained new pure composition fixture: {}", root.display());
}
#[test]
fn capacity_without_claim_authority_and_stale_preparation_remain_blocked() {
    let mut kernel = initial_kernel();
    let evaluation = declared(&kernel);
    let scope = kernel.state().readiness().scope(&task("A")).unwrap();
    let prepared = prepare(
        &kernel,
        kernel.revision(),
        &[proposal(&evaluation, "A")],
        &[TaskOperations {
            task: task("A"),
            candidate: scope.context().contract().candidate().clone(),
            operations: None,
        }],
    )
    .unwrap();
    assert!(prepared.operations().is_empty());
    assert!(prepared.decisions()[0].coordination_hold.is_some());
    let stale = prepare_task(&kernel, &evaluation, "A");
    let independent = pair(&kernel, "D")[0].clone();
    kernel.admit(independent).unwrap();
    let count = kernel.revision();
    assert!(kernel.admit(stale[0].clone()).is_err());
    assert_eq!(kernel.revision(), count);
    assert!(kernel.fixture_effects().effects().is_empty());
}
#[test]
fn unknown_success_cannot_be_hidden_by_composition() {
    let mut kernel = initial_kernel();
    let mut evaluation = declared(&kernel);
    let a = join(&mut kernel, &mut evaluation, "A");
    unknown(&mut kernel, &a);
    assert!(
        evaluation
            .record(&kernel, observation(&evaluation, &a, exited(0)))
            .is_err()
    );
    let unknown_observation = observation(&evaluation, &a, AttemptExecution::Unknown);
    evaluation.record(&kernel, unknown_observation).unwrap();
    assert!(prepare_task(&kernel, &evaluation, "B").is_empty());
    let result = inspect(&mut evaluation, &kernel);
    assert_eq!(result.evaluation.arms[0].population.unknown, 1);
    assert_eq!(result.evaluation.arms[0].population.completed, 0);
    assert_eq!(
        result.evaluation.arms[0]
            .trials
            .iter()
            .find(|t| t.trial.as_str() == "trial-B")
            .unwrap()
            .attempts,
        0
    );
    assert!(!result.evaluation.primary_outcome_available);
}

fn join(
    kernel: &mut CoordinationKernel,
    evaluation: &mut ProgramEvaluation,
    name: &str,
) -> ProgramAttempt {
    let attempt = prepare_attempt(kernel, evaluation, name);
    evaluation
        .register_attempt(kernel, attempt.clone())
        .unwrap();
    attempt
}
#[test]
fn omitted_producer_remains_visible_after_successful_composed_consumer() {
    let mut k = initial_kernel();
    let mut e = declared(&k);
    let a = prepare_attempt(&mut k, &e, "A");
    report(&mut k, &a, EffectOutcome::Succeeded);
    release(&mut k, "AB");
    let b = join(&mut k, &mut e, "B");
    report(&mut k, &b, EffectOutcome::Succeeded);
    e.record(&k, observation(&e, &b, exited(0))).unwrap();
    let r = inspect(&mut e, &k);
    assert!(
        r.coverage
            .contains(&ProgramCoverageIssue::UnmappedAttempt(a.scope.attempt))
    );
    assert!(
        r.coverage
            .contains(&ProgramCoverageIssue::UnmappedEffect(a.effect))
    );
    assert_eq!(r.evaluation.arms[0].population.completed, 1);
    assert_eq!(
        r.evaluation.eligibility,
        ComparisonEligibility::NotComparable
    );
    assert!(!r.evaluation.primary_outcome_available);
}
#[test]
fn unobserved_composed_predecessor_blocks_retry_registration() {
    let mut k = initial_kernel();
    let mut e = declared(&k);
    let a = join(&mut k, &mut e, "A");
    report(&mut k, &a, EffectOutcome::Failed);
    settle_for_retry(&mut k, &a, EffectOutcome::Failed);
    let retry = reserve_attempt(
        &mut k,
        "separately-authorized-retry",
        "A",
        Some(a.definition.id.as_str()),
    );
    assert!(e.register_attempt(&k, retry).is_err());
    assert_eq!(e.ledger().attempts().len(), 1);
    assert!(e.ledger().attempts()[0].observation().is_none());
}
#[test]
fn synthetic_composed_result_cannot_claim_live_configuration() {
    let mut k = initial_kernel();
    let mut e = declared(&k);
    let a = join(&mut k, &mut e, "A");
    report(&mut k, &a, EffectOutcome::Succeeded);
    let mut o = observation(&e, &a, exited(0));
    o.context.configuration.profile = cortex_core::evaluation::ExecutionProfile::LiveProvider;
    assert!(e.record(&k, o).is_err());
    assert!(e.ledger().attempts()[0].observation().is_none());
    assert!(!inspect(&mut e, &k).evaluation.primary_outcome_available);
}
#[test]
fn local_composed_endpoint_cannot_claim_protected_integration() {
    let mut k = initial_kernel();
    let mut e = declared(&k);
    let a = join(&mut k, &mut e, "A");
    report(&mut k, &a, EffectOutcome::Succeeded);
    let mut o = observation(&e, &a, exited(0));
    let mut endpoint = model::endpoint(&e, &a);
    endpoint.acceptance.as_mut().unwrap().scope = ResultScope::Integrated;
    o.endpoint = Some(endpoint);
    assert!(matches!(
        e.record(&k, o),
        Err(ProgramEvaluationError::Outcome)
    ));
    assert!(e.ledger().attempts()[0].observation().is_none());
    assert_eq!(
        inspect(&mut e, &k).evaluation.arms[0].reported_accepted_trials,
        0
    );
}
fn method_hold(stale: bool, exception: bool) {
    use cortex_core::binding::*;
    use cortex_evidence::methods::proposal::MethodProposalError;
    let kernel = initial_kernel();
    let mut evaluation = declared(&kernel);
    let before = kernel.operations().to_vec();
    let revision = kernel.revision();
    let scope = kernel.state().readiness().scope(&task("A")).unwrap();
    let contract = scope.context().contract();
    let mut package = match &pair(&kernel, "A")[1].command {
        Command::Package(p) => p.clone(),
        _ => unreachable!(),
    };
    package.methods.clear();
    let current = if stale {
        CandidateObservation::new(
            ObservationId::new(contract.task().id().as_str(), 3).unwrap(),
            contract,
            RepositoryState::new(
                Some(contract.candidate().repository().clone()),
                Head::Detached {
                    commit: contract.candidate().commit().clone(),
                },
                vec![Entry::Untracked {
                    path: b"changed-source".to_vec(),
                }],
                ObservationCoverage::GitStatus,
                ObservationConsistency::TwoMatchingSamples,
            )
            .unwrap(),
            scope.context().capture().unwrap().basis().cloned(),
        )
    } else {
        capture(contract, 2)
    };
    let profile = oracles(&specification(&kernel))[0].profile.clone();
    let result = composition::advisory_package_result(
        kernel.state(),
        package,
        &current,
        &profile,
        exception,
    );
    if stale {
        assert!(matches!(
            result,
            Err(MethodProposalError::TargetNotCurrent(_))
        ));
    } else {
        let (package, count) = result.unwrap();
        assert_eq!(count, 1);
        assert!(package.methods[0].qualification.is_none());
        assert!(
            package.methods[0]
                .exceptions
                .contains("unresolved-protected-exception")
        );
    }
    // Explicit caller withholds authority; advisory method inspection grants none.
    let held = prepare(
        &kernel,
        revision,
        &[proposal(&evaluation, "A")],
        &[TaskOperations {
            task: task("A"),
            candidate: contract.candidate().clone(),
            operations: None,
        }],
    )
    .unwrap();
    assert!(held.operations().is_empty());
    assert!(held.decisions()[0].coordination_hold.is_some());
    let report = inspect(&mut evaluation, &kernel);
    assert_eq!(report.evaluation.arms[0].population.registered, 0);
    assert_eq!(report.evaluation.arms[0].trials.len(), 6);
    assert!(
        report.evaluation.arms[0]
            .trials
            .iter()
            .all(|t| t.attempts == 0)
    );
    assert!(!report.evaluation.primary_outcome_available);
    assert_eq!(prepare_task(&kernel, &evaluation, "D").len(), 2);
    assert_eq!(kernel.revision(), revision);
    assert!(kernel.operations() == before);
    assert!(kernel.state().attempts().is_empty());
    assert!(kernel.packages().is_empty());
    assert!(kernel.fixture_effects().effects().is_empty());
}
#[test]
fn structurally_matching_stale_method_cannot_enter_composed_preparation() {
    method_hold(true, false);
}
#[test]
fn unresolved_method_exception_preserves_explicit_composition_hold() {
    method_hold(false, true);
}
