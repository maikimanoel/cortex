//! Canonical operational fixture composition and immutable replay boundaries.
#[path = "support/program.rs"]
mod fixture;
#[path = "support/operational.rs"]
mod operational_fixture;
use cortex_core::program::{operational::*, *};
use cortex_evidence::program::*;
use operational_fixture::*;

#[test]
fn fixture_journal_replay_is_idempotent_and_changed_delivery_is_atomic() {
    let (mut k, s) = fixture_kernel();
    let input = operation(
        &k,
        "reserve",
        Command::FixtureEffect(Box::new(EffectEvent::Reserve {
            intent: intent(&s, "effect"),
            at: 1,
        })),
    );
    k.admit(input.clone()).unwrap();
    let count = k.revision();
    assert!(k.admit(input.clone()).unwrap().duplicate);
    let mut changed = input;
    changed.expected = count;
    assert!(k.admit(changed).is_err());
    assert_eq!(k.revision(), count);
    let mut replay = CoordinationKernel::new(k.initial().clone()).unwrap();
    for op in k.operations() {
        replay.admit(op.clone()).unwrap();
    }
    assert!(replay.fixture_effects() == k.fixture_effects());
    assert_eq!(
        replay.fixture_effects().effects()[0].status,
        EffectStatus::Reserved
    );
    assert!(replay.records().is_empty());
}

#[test]
fn unknown_effect_prevents_attempt_retirement_and_preserves_original_failure() {
    let (mut k, s) = fixture_kernel();
    effect(
        &mut k,
        "reserve",
        EffectEvent::Reserve {
            intent: intent(&s, "effect"),
            at: 1,
        },
    );
    effect(
        &mut k,
        "dispatch",
        EffectEvent::Dispatch {
            effect: EffectId::new("effect").unwrap(),
            scope: s.clone(),
            at: 2,
            basis: source("dispatch-boundary"),
        },
    );
    effect(
        &mut k,
        "unknown",
        EffectEvent::Unknown {
            effect: EffectId::new("effect").unwrap(),
            scope: s.clone(),
            at: 3,
            basis: source("reply-lost"),
        },
    );
    for status in [AttemptStatus::Reported, AttemptStatus::Cancelled] {
        let before = k.revision();
        assert!(
            k.admit(operation(
                &k,
                "retire",
                Command::Attempt {
                    id: s.attempt.clone(),
                    generation: s.generation,
                    status,
                    basis: source("not-cleanup")
                }
            ))
            .is_err()
        );
        assert_eq!(k.revision(), before);
    }
    effect(
        &mut k,
        "report",
        EffectEvent::Report {
            effect: EffectId::new("effect").unwrap(),
            scope: s.clone(),
            at: 4,
            outcome: EffectOutcome::Failed,
            basis: source("observed-failure"),
        },
    );
    effect(
        &mut k,
        "settle",
        EffectEvent::Reconcile {
            effect: EffectId::new("effect").unwrap(),
            scope: s.clone(),
            at: 5,
            resolution: Reconciliation::Observed {
                outcome: EffectOutcome::Failed,
                usage: demand(),
            },
            basis: source("exact-outcome"),
        },
    );
    assert_eq!(
        k.fixture_effects().effects()[0].report,
        Some(EffectOutcome::Failed)
    );
    admit(
        &mut k,
        "retire",
        Command::Attempt {
            id: s.attempt,
            generation: s.generation,
            status: AttemptStatus::Cancelled,
            basis: source("fixture-settled"),
        },
    );
    assert!(k.records().is_empty());
}

#[test]
fn absent_or_wrong_package_and_unscoped_policy_are_refused_without_partial_reservation() {
    let (mut k, s) = fixture_kernel();
    let mut bad = intent(&s, "effect");
    bad.scope.package = PackageId::new("missing").unwrap();
    assert!(
        k.admit(operation(
            &k,
            "bad",
            Command::FixtureEffect(Box::new(EffectEvent::Reserve { intent: bad, at: 1 }))
        ))
        .is_err()
    );
    assert!(k.fixture_effects().effects().is_empty());
    let mut new_policy = policy();
    new_policy.reference = source("policy2");
    let mut input = operation(
        &k,
        "policy2",
        Command::FixtureEffect(Box::new(EffectEvent::Policy(new_policy))),
    );
    input.grant.id = GrantId::new("narrow").unwrap();
    input.grant.tasks = vec![task("A")];
    assert!(k.admit(input).is_err());
    assert_eq!(k.fixture_effects().policies().len(), 1);
}

#[test]
fn prepared_executor_input_retains_exact_package_and_cannot_grant_execution() {
    let (mut k, s) = fixture_kernel();
    admit(
        &mut k,
        "prepare",
        Command::PrepareExecutorInput(Box::new(preparation(&s))),
    );
    let input = &k.prepared_inputs()[0].1;
    assert_eq!(input.bytes(), k.packages()[0].bytes());
    input
        .check(k.state(), &descriptor(), &requirements(), input.bytes())
        .unwrap();
    let mut replay = CoordinationKernel::new(k.initial().clone()).unwrap();
    for operation in k.operations() {
        replay.admit(operation.clone()).unwrap();
    }
    assert!(replay.prepared_inputs() == k.prepared_inputs());
    assert!(replay.fixture_effects().effects().is_empty());
    assert!(replay.records().is_empty());
}

#[test]
fn retained_executor_and_requirement_identities_cannot_alias_changed_contents() {
    use cortex_core::program::executor::*;
    let (mut k, s) = fixture_kernel();
    admit(
        &mut k,
        "prepare",
        Command::PrepareExecutorInput(Box::new(preparation(&s))),
    );
    let before = k.revision();
    let old = descriptor();
    let mut request = preparation(&s);
    request.descriptor = ExecutorDescriptor::new(
        old.reference().clone(),
        ExecutorIdentity::new(
            "fixture-executor",
            "2",
            source("different-binary"),
            source("synthetic-host"),
        )
        .unwrap(),
        old.transport(),
        old.observations().to_vec(),
        2,
    )
    .unwrap();
    assert!(
        k.admit(operation(
            &k,
            "alias",
            Command::PrepareExecutorInput(Box::new(request.clone()))
        ))
        .is_err()
    );
    assert_eq!(k.revision(), before);
    request.descriptor = ExecutorDescriptor::new(
        source("descriptor-v2"),
        request.descriptor.identity().clone(),
        old.transport(),
        old.observations().to_vec(),
        2,
    )
    .unwrap();
    admit(
        &mut k,
        "new-descriptor",
        Command::PrepareExecutorInput(Box::new(request)),
    );
    let mut request = preparation(&s);
    request.requirements = Requirements::new(
        requirements().reference().clone(),
        vec![
            Requirement {
                control: Control::StandardInput,
                kind: ObservationKind::ControlledFixture,
            },
            Requirement {
                control: Control::StructuredOutput,
                kind: ObservationKind::Advertised,
            },
        ],
        2,
    )
    .unwrap();
    assert!(matches!(
        k.admit(operation(
            &k,
            "profile-alias",
            Command::PrepareExecutorInput(Box::new(request))
        )),
        Err(KernelError::Core(ProgramError::IdentityConflict))
    ));
}
