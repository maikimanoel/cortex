//! Canonical supervised-fixture composition. No process is launched by these tests.
#[path = "support/program.rs"]
mod fixture;
#[path = "support/supervision.rs"]
mod support;
use cortex_core::program::{operational::*, supervision::*, *};
use cortex_evidence::program::*;
use support::*;

#[test]
fn supervised_commands_require_explicit_kernel_profile_and_old_commands_cannot_cross_it() {
    let (mut supervised, _, _) = configured_kernel(None);
    let mut pure = CoordinationKernel::new(supervised.initial().clone()).unwrap();
    assert!(
        pure.admit(operation(
            &pure,
            "supervised-in-old",
            Command::SupervisedEffect(Box::new(EffectEvent::Policy(policy())))
        ))
        .is_err()
    );
    assert!(pure.operations().is_empty());
    let before = supervised.revision();
    assert!(
        supervised
            .admit(operation(
                &supervised,
                "old-in-supervised",
                Command::FixtureEffect(Box::new(EffectEvent::Policy(policy())))
            ))
            .is_err()
    );
    assert_eq!(supervised.revision(), before);
}
#[test]
fn process_preparation_binds_exact_retained_input_candidate_policy_and_approval() {
    let (mut k, s, spec) = configured_kernel(None);
    effect(
        &mut k,
        "reserve",
        EffectEvent::Reserve {
            intent: intent(&s),
            at: 1,
        },
    );
    for case in 0..8 {
        let mut changed = spec.clone();
        match case {
            0 => changed.prepared_input = OperationId::new("missing-prepared").unwrap(),
            1 => changed.input_len += 1,
            2 => changed.cwd = changed.cwd.join("other-workspace"),
            3 => changed.executable_identity = source("different-executable"),
            4 => changed.profile = source("different-profile"),
            5 => changed.host = source("different-host"),
            6 => changed.privacy = source("different-privacy"),
            _ => changed.approval = source("different-approval"),
        }
        let before = k.revision();
        assert!(
            k.admit(operation(
                &k,
                "bad-spec",
                Command::SupervisedEffect(Box::new(EffectEvent::PrepareProcess {
                    effect: intent(&s).id,
                    scope: s.clone(),
                    at: 2,
                    spec: Box::new(changed)
                }))
            ))
            .is_err()
        );
        assert_eq!(k.revision(), before);
        assert!(k.fixture_effects().process_spec(&intent(&s).id).is_none());
    }
    effect(
        &mut k,
        "spec",
        EffectEvent::PrepareProcess {
            effect: intent(&s).id,
            scope: s.clone(),
            at: 2,
            spec: Box::new(spec),
        },
    );
    k.admit(dispatch_operation(&k, &s)).unwrap();
    assert_eq!(
        k.fixture_effects().effects()[0].status,
        EffectStatus::DispatchUncertain
    );
}
#[test]
fn exact_dispatch_replay_and_process_receipt_replay_do_not_create_more_intents() {
    let (mut k, s, spec) = prepared_kernel(None);
    let dispatch = dispatch_operation(&k, &s);
    k.admit(dispatch.clone()).unwrap();
    let n = k.revision();
    assert!(k.admit(dispatch).unwrap().duplicate);
    assert_eq!(k.revision(), n);
    let spawned = observation(&spec, 1, ProcessCompletion::Spawned, Quiescence::Unknown);
    let input = operation(
        &k,
        "spawned",
        Command::SupervisedEffect(Box::new(EffectEvent::ProcessObserved {
            effect: intent(&s).id,
            scope: s.clone(),
            at: 4,
            observation: spawned.clone(),
        })),
    );
    k.admit(input.clone()).unwrap();
    assert!(k.admit(input).unwrap().duplicate);
    let before = k.revision();
    assert!(
        k.admit(operation(
            &k,
            "reused-observation",
            Command::SupervisedEffect(Box::new(EffectEvent::ProcessObserved {
                effect: intent(&s).id,
                scope: s.clone(),
                at: 5,
                observation: spawned
            }))
        ))
        .is_err()
    );
    assert_eq!(k.revision(), before);
    let mut replay = CoordinationKernel::new_supervised_fixture(k.initial().clone()).unwrap();
    for input in k.operations() {
        replay.admit(input.clone()).unwrap();
    }
    assert!(replay.fixture_effects() == k.fixture_effects());
    assert_eq!(replay.fixture_effects().effects().len(), 1);
    assert!(replay.records().is_empty());
}
#[test]
fn unknown_cleanup_blocks_retirement_until_captured_quiescence_and_exact_settlement() {
    let (mut k, s, spec) = prepared_kernel(None);
    k.admit(dispatch_operation(&k, &s)).unwrap();
    let spawned = observation(&spec, 1, ProcessCompletion::Spawned, Quiescence::Unknown);
    effect(
        &mut k,
        "spawned",
        EffectEvent::ProcessObserved {
            effect: intent(&s).id,
            scope: s.clone(),
            at: 4,
            observation: spawned,
        },
    );
    effect(
        &mut k,
        "cancel",
        EffectEvent::Cancel {
            effect: intent(&s).id,
            scope: s.clone(),
            at: 5,
            basis: source("request"),
        },
    );
    let unknown = observation(
        &spec,
        2,
        ProcessCompletion::Interrupted {
            reason: InterruptionReason::Cancelled,
        },
        Quiescence::Unknown,
    );
    effect(
        &mut k,
        "cleanup-unknown",
        EffectEvent::ProcessObserved {
            effect: intent(&s).id,
            scope: s.clone(),
            at: 6,
            observation: unknown,
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
                    basis: source("not-yet")
                }
            ))
            .is_err()
        );
        assert_eq!(k.revision(), before);
    }
    let done = observation(
        &spec,
        3,
        ProcessCompletion::Interrupted {
            reason: InterruptionReason::Cancelled,
        },
        Quiescence::DirectChildReapedStreamsClosed,
    );
    effect(
        &mut k,
        "cleanup-known",
        EffectEvent::ProcessObserved {
            effect: intent(&s).id,
            scope: s.clone(),
            at: 7,
            observation: done.clone(),
        },
    );
    effect(
        &mut k,
        "settled",
        EffectEvent::Reconcile {
            effect: intent(&s).id,
            scope: s.clone(),
            at: 8,
            resolution: Reconciliation::Observed {
                outcome: EffectOutcome::Interrupted,
                usage: usage(&done),
            },
            basis: source("exact-observed-cleanup"),
        },
    );
    admit(
        &mut k,
        "retire",
        Command::Attempt {
            id: s.attempt.clone(),
            generation: s.generation,
            status: AttemptStatus::Cancelled,
            basis: source("settled-retirement"),
        },
    );
    assert!(k.fixture_effects().effects()[0].cancellation_requested);
    assert_eq!(
        k.fixture_effects().effects()[0].process_observations.len(),
        3
    );
    assert!(k.records().is_empty());
}
#[test]
fn a_process_observation_cannot_supply_its_own_coordination_scope() {
    let (mut k, s, spec) = prepared_kernel(None);
    k.admit(dispatch_operation(&k, &s)).unwrap();
    let o = observation(&spec, 1, ProcessCompletion::Spawned, Quiescence::Unknown);
    let mut input = operation(
        &k,
        "unauthorized-observation",
        Command::SupervisedEffect(Box::new(EffectEvent::ProcessObserved {
            effect: intent(&s).id,
            scope: s.clone(),
            at: 4,
            observation: o,
        })),
    );
    input.grant.id = GrantId::new("different-mission-grant").unwrap();
    input.grant.tasks = vec![task("D")];
    let before = k.revision();
    assert!(k.admit(input).is_err());
    assert_eq!(k.revision(), before);
    assert!(
        k.fixture_effects().effects()[0]
            .process_observations
            .is_empty()
    );
}

#[test]
fn historical_unestablished_memory_reconstructs_but_cannot_prepare_or_dispatch_again() {
    use cortex_core::program::scheduling::Resource;
    let (mut current, scope, spec) = configured_scope_kernel(None);
    prepare(&mut current, &scope, &spec);
    let mut history = current.operations().to_vec();
    for operation in &mut history {
        if let Command::SupervisedEffect(event) = &mut operation.command {
            match event.as_mut() {
                EffectEvent::Policy(policy) => {
                    for capacity in &mut policy.capacities {
                        if matches!(capacity.resource, Resource::MemoryBytes(_)) {
                            capacity.resource = Resource::UnspecifiedMemoryBytes;
                        }
                    }
                }
                EffectEvent::Grant(grant) => {
                    for demand in &mut grant.ceilings {
                        if matches!(demand.resource, Resource::MemoryBytes(_)) {
                            demand.resource = Resource::UnspecifiedMemoryBytes;
                        }
                    }
                }
                EffectEvent::Reserve { intent, .. } => {
                    for demand in &mut intent.demand {
                        if matches!(demand.resource, Resource::MemoryBytes(_)) {
                            demand.resource = Resource::UnspecifiedMemoryBytes;
                        }
                    }
                }
                EffectEvent::PrepareProcess { spec, .. } => {
                    spec.containment.as_mut().unwrap().memory_resource = None
                }
                _ => {}
            }
        }
    }
    for prepared in [false, true] {
        let count = history.len() - usize::from(!prepared);
        let mut inputs = history[..count].iter().cloned();
        let mut restored = CoordinationKernel::new_supervised_fixture(current.initial().clone())
            .unwrap()
            .reconstruct(|_| Ok::<_, KernelError>(inputs.next()))
            .unwrap();
        assert!(restored.operations() == &history[..count]);
        assert!(
            restored
                .admit(history[count - 1].clone())
                .unwrap()
                .duplicate
        );
        let next = if prepared {
            dispatch_operation(&restored, &scope)
        } else {
            let mut legacy_spec = spec.clone();
            legacy_spec.containment.as_mut().unwrap().memory_resource = None;
            operation(
                &restored,
                "new-prepare",
                Command::SupervisedEffect(Box::new(EffectEvent::PrepareProcess {
                    effect: intent(&scope).id,
                    scope: scope.clone(),
                    at: 2,
                    spec: Box::new(legacy_spec),
                })),
            )
        };
        let before = restored.revision();
        assert!(matches!(
            restored.admit(next),
            Err(KernelError::Operational(OperationalError::UnknownResource(
                Resource::UnspecifiedMemoryBytes
            )))
        ));
        assert_eq!(restored.revision(), before);
        assert!(restored.fixture_effects().effects()[0].pending());
        if prepared {
            // The legacy inspection remains an advisory projection. Retaining
            // its old raw quantity cannot authorize a new settlement today.
            let mut dispatched_history = history.clone();
            dispatched_history.push(dispatch_operation(&restored, &scope));
            let mut inputs = dispatched_history.into_iter();
            let mut dispatched =
                CoordinationKernel::new_supervised_fixture(current.initial().clone())
                    .unwrap()
                    .reconstruct(|_| Ok::<_, KernelError>(inputs.next()))
                    .unwrap();
            let mut terminal = observation(
                &spec,
                1,
                ProcessCompletion::Exited {
                    code: Some(0),
                    signal: None,
                },
                Quiescence::OwnedScopeEmpty,
            );
            terminal.containment = Some(Box::new(scope_observation(
                spec.containment.as_ref().unwrap(),
                Some(false),
            )));
            effect(
                &mut dispatched,
                "legacy-terminal",
                EffectEvent::ProcessObserved {
                    effect: intent(&scope).id,
                    scope: scope.clone(),
                    at: 4,
                    observation: terminal,
                },
            );
            let recovery = dispatched
                .fixture_effects()
                .inspect_process_recovery(&intent(&scope).id, &scope)
                .unwrap();
            assert!(recovery.blockers.is_empty());
            let resolution = recovery
                .candidate
                .expect("legacy advisory projection retained");
            let prior = dispatched.revision();
            let reconcile = operation(
                &dispatched,
                "new-reconcile",
                Command::SupervisedEffect(Box::new(EffectEvent::Reconcile {
                    effect: intent(&scope).id,
                    scope: scope.clone(),
                    at: 5,
                    resolution,
                    basis: source("legacy-proposal-is-not-admission"),
                })),
            );
            assert!(matches!(
                dispatched.admit(reconcile),
                Err(KernelError::Operational(OperationalError::UnknownResource(
                    Resource::UnspecifiedMemoryBytes
                )))
            ));
            assert_eq!(dispatched.revision(), prior);
            assert!(dispatched.fixture_effects().effects()[0].pending());
        }
    }
}
