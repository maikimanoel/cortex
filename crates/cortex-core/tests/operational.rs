//! Stage 2A fixture contracts discriminate unsafe retry, stale authority and budget reuse.
#[path = "support/program.rs"]
mod fixture;
use cortex_core::{
    binding::*,
    parallel::*,
    program::{operational::*, scheduling::*, *},
};
use fixture::*;

fn policy() -> Policy {
    Policy {
        reference: source("fixture-policy"),
        program: ProgramId::new("fixture-program").unwrap(),
        baseline: rev(1),
        mode: EffectMode::ControlledFixture,
        host: source("declared-fixture-host"),
        privacy: source("explicit-fixture-only-data"),
        capacities: vec![
            Capacity {
                resource: Resource::WorkerSlots,
                available: Some(2),
                verification_reserve: 1,
                declared_enforceable: true,
            },
            Capacity {
                resource: Resource::ModelTokens,
                available: Some(100),
                verification_reserve: 20,
                declared_enforceable: true,
            },
        ],
        cumulative: vec![Resource::ModelTokens],
        max_effects: 8,
        max_retries: 2,
    }
}
fn scope(state: &ProgramState, task: &str) -> EffectScope {
    EffectScope {
        program: state.definition().id().clone(),
        baseline: state.definition().revision(),
        task: cortex_core::TaskId::new(task).unwrap(),
        candidate: state
            .readiness()
            .scope(&cortex_core::TaskId::new(task).unwrap())
            .unwrap()
            .context()
            .contract()
            .candidate()
            .clone(),
        attempt: AttemptId::new(&format!("attempt-{task}")).unwrap(),
        generation: rev(1),
        package: PackageId::new(&format!("package-{task}")).unwrap(),
        package_revision: rev(1),
    }
}
fn demand(tokens: u64) -> Vec<Demand> {
    vec![
        Demand {
            resource: Resource::WorkerSlots,
            amount: Some(1),
        },
        Demand {
            resource: Resource::ModelTokens,
            amount: Some(tokens),
        },
    ]
}
fn operational_grant(state: &ProgramState, task: &str) -> OperationalGrant {
    OperationalGrant {
        id: GrantId::new(&format!("effects-{task}")).unwrap(),
        principal: ActorId::new("operator").unwrap(),
        policy: policy().reference,
        scope: scope(state, task),
        not_before: 10,
        expires_before: 100,
        ceilings: demand(60),
        basis: source("fixture-grant"),
    }
}
fn intent(state: &ProgramState, task: &str, id: &str) -> EffectIntent {
    EffectIntent {
        id: EffectId::new(id).unwrap(),
        grant: operational_grant(state, task).id,
        scope: scope(state, task),
        demand: demand(50),
        retry_of: None,
    }
}
fn setup() -> (ProgramState, OperationalProjection) {
    let mut state = ProgramState::new(definition()).unwrap();
    initialize(&mut state);
    state
        .claim(&grant(1, "operator"), attempt("attempt-A", "A", "operator"))
        .unwrap();
    state
        .claim(&grant(1, "operator"), attempt("attempt-D", "D", "operator"))
        .unwrap();
    let mut projection = OperationalProjection::new();
    projection
        .apply(&state, EffectEvent::Policy(policy()))
        .unwrap();
    projection
        .apply(&state, EffectEvent::Grant(operational_grant(&state, "A")))
        .unwrap();
    projection
        .apply(&state, EffectEvent::Grant(operational_grant(&state, "D")))
        .unwrap();
    (state, projection)
}
fn reserve(state: &ProgramState, p: &mut OperationalProjection, task: &str, id: &str, at: u64) {
    p.apply(
        state,
        EffectEvent::Reserve {
            intent: intent(state, task, id),
            at,
        },
    )
    .unwrap();
}
fn dispatch(state: &ProgramState, p: &mut OperationalProjection, task: &str, id: &str, at: u64) {
    p.apply(
        state,
        EffectEvent::Dispatch {
            effect: EffectId::new(id).unwrap(),
            scope: scope(state, task),
            at,
            basis: source("dispatch-boundary"),
        },
    )
    .unwrap();
}
fn absent(state: &ProgramState, p: &mut OperationalProjection, task: &str, id: &str, at: u64) {
    p.apply(
        state,
        EffectEvent::Reconcile {
            effect: EffectId::new(id).unwrap(),
            scope: scope(state, task),
            at,
            resolution: Reconciliation::Absent,
            basis: source("exact-fixture-no-effect"),
        },
    )
    .unwrap();
}

#[test]
fn no_policy_or_external_mode_cannot_create_effect_authority() {
    let (state, _) = setup();
    let mut p = OperationalProjection::new();
    assert!(
        p.apply(&state, EffectEvent::Grant(operational_grant(&state, "A")))
            .is_err()
    );
    let mut external = policy();
    external.mode = EffectMode::ExternalExecutor;
    assert_eq!(
        p.apply(&state, EffectEvent::Policy(external)),
        Err(OperationalError::ExternalUnavailable)
    );
    assert!(p.events().is_empty());
    assert_eq!(state.authority(), ExecutionAuthority::Unavailable);
}
#[test]
fn unknown_capacity_or_unenforceable_requirement_is_held_without_mutation() {
    let (state, _) = setup();
    for unknown in [true, false] {
        let mut p = OperationalProjection::new();
        let mut selected = policy();
        if unknown {
            selected.capacities[1].available = None;
        } else {
            selected.capacities[1].declared_enforceable = false;
        }
        p.apply(&state, EffectEvent::Policy(selected)).unwrap();
        p.apply(&state, EffectEvent::Grant(operational_grant(&state, "A")))
            .unwrap();
        let before = p.clone();
        let e = p.apply(
            &state,
            EffectEvent::Reserve {
                intent: intent(&state, "A", "one"),
                at: 10,
            },
        );
        assert_eq!(
            e,
            Err(if unknown {
                OperationalError::UnknownResource(Resource::ModelTokens)
            } else {
                OperationalError::UnenforceableResource(Resource::ModelTokens)
            })
        );
        assert!(p == before);
    }
}
#[test]
fn concurrent_intents_cannot_spend_verification_reserve_and_rejection_is_atomic() {
    let (state, mut p) = setup();
    reserve(&state, &mut p, "A", "one", 10);
    let before = p.clone();
    assert_eq!(
        p.apply(
            &state,
            EffectEvent::Reserve {
                intent: intent(&state, "D", "two"),
                at: 11
            }
        ),
        Err(OperationalError::ResourceLimit(Resource::WorkerSlots))
    );
    assert!(p == before);
    p.apply(
        &state,
        EffectEvent::Cancel {
            effect: EffectId::new("one").unwrap(),
            scope: scope(&state, "A"),
            at: 11,
            basis: source("before-dispatch"),
        },
    )
    .unwrap();
    reserve(&state, &mut p, "D", "two", 12);
    assert_eq!(p.accounted(Resource::WorkerSlots).unwrap(), 1);
}
#[test]
fn dispatch_is_uncertain_and_replay_cannot_dispatch_again() {
    let (state, mut p) = setup();
    reserve(&state, &mut p, "A", "one", 10);
    dispatch(&state, &mut p, "A", "one", 11);
    assert_eq!(p.effects()[0].status, EffectStatus::DispatchUncertain);
    let before = p.clone();
    assert_eq!(
        p.apply(
            &state,
            EffectEvent::Dispatch {
                effect: EffectId::new("one").unwrap(),
                scope: scope(&state, "A"),
                at: 12,
                basis: source("duplicate")
            }
        ),
        Err(ProgramError::Transition.into())
    );
    assert!(p == before);
    assert!(p.has_pending_attempt(&scope(&state, "A").attempt));
}
#[test]
fn lost_reply_revocation_and_cancel_do_not_refund_unknown_effects() {
    let (state, mut p) = setup();
    reserve(&state, &mut p, "A", "one", 10);
    dispatch(&state, &mut p, "A", "one", 11);
    p.apply(
        &state,
        EffectEvent::Unknown {
            effect: EffectId::new("one").unwrap(),
            scope: scope(&state, "A"),
            at: 12,
            basis: source("lost-reply"),
        },
    )
    .unwrap();
    p.apply(
        &state,
        EffectEvent::Revoke {
            grant: operational_grant(&state, "A").id,
            at: 13,
            basis: source("revocation"),
        },
    )
    .unwrap();
    p.apply(
        &state,
        EffectEvent::Cancel {
            effect: EffectId::new("one").unwrap(),
            scope: scope(&state, "A"),
            at: 14,
            basis: source("cancellation-request"),
        },
    )
    .unwrap();
    assert_eq!(p.effects()[0].status, EffectStatus::CancellationRequested);
    assert_eq!(p.accounted(Resource::WorkerSlots).unwrap(), 1);
    assert!(
        p.apply(
            &state,
            EffectEvent::Reserve {
                intent: intent(&state, "D", "other"),
                at: 15
            }
        )
        .is_err()
    );
    absent(&state, &mut p, "A", "one", 15);
    reserve(&state, &mut p, "D", "other", 16);
    assert!(
        p.events()
            .iter()
            .any(|e| matches!(e, EffectEvent::Unknown { .. }))
    );
}
#[test]
fn grant_expiry_revocation_and_clock_regression_hold_dispatch() {
    for case in 0..3 {
        let (state, mut p) = setup();
        reserve(&state, &mut p, "A", "one", 10);
        if case == 1 {
            p.apply(
                &state,
                EffectEvent::Revoke {
                    grant: operational_grant(&state, "A").id,
                    at: 11,
                    basis: source("revoke"),
                },
            )
            .unwrap();
        }
        let at = if case == 0 {
            100
        } else if case == 1 {
            12
        } else {
            9
        };
        let before = p.clone();
        assert_eq!(
            p.apply(
                &state,
                EffectEvent::Dispatch {
                    effect: EffectId::new("one").unwrap(),
                    scope: scope(&state, "A"),
                    at,
                    basis: source("try")
                }
            ),
            Err(if case == 2 {
                OperationalError::ClockRegression
            } else {
                OperationalError::GrantUnavailable
            })
        );
        assert!(p == before);
    }
}
#[test]
fn stale_baseline_blocks_dispatch_but_historical_unknown_can_be_reconciled() {
    let (mut state, mut p) = setup();
    let old = scope(&state, "A");
    reserve(&state, &mut p, "A", "one", 10);
    dispatch(&state, &mut p, "A", "one", 11);
    state
        .amend(&grant(1, "operator"), definition_revision(2, true))
        .unwrap();
    assert!(
        p.apply(
            &state,
            EffectEvent::Report {
                effect: EffectId::new("one").unwrap(),
                scope: old.clone(),
                at: 12,
                outcome: EffectOutcome::Succeeded,
                basis: source("late-report")
            }
        )
        .is_err()
    );
    p.apply(
        &state,
        EffectEvent::Reconcile {
            effect: EffectId::new("one").unwrap(),
            scope: old,
            at: 12,
            resolution: Reconciliation::Absent,
            basis: source("historical-exact-reconcile"),
        },
    )
    .unwrap();
    assert_eq!(p.effects()[0].status, EffectStatus::ReconciledAbsent);
    assert_eq!(state.attempts()[0].status, AttemptStatus::Claimed);
}
#[test]
fn report_is_not_acceptance_or_release_and_cannot_be_rewritten() {
    let (state, mut p) = setup();
    reserve(&state, &mut p, "A", "one", 10);
    dispatch(&state, &mut p, "A", "one", 11);
    p.apply(
        &state,
        EffectEvent::Report {
            effect: EffectId::new("one").unwrap(),
            scope: scope(&state, "A"),
            at: 12,
            outcome: EffectOutcome::Failed,
            basis: source("failure"),
        },
    )
    .unwrap();
    assert_eq!(p.accounted(Resource::WorkerSlots).unwrap(), 1);
    assert_eq!(state.attempts()[0].status, AttemptStatus::Claimed);
    assert!(
        p.apply(
            &state,
            EffectEvent::Reconcile {
                effect: EffectId::new("one").unwrap(),
                scope: scope(&state, "A"),
                at: 13,
                resolution: Reconciliation::Observed {
                    outcome: EffectOutcome::Succeeded,
                    usage: demand(40)
                },
                basis: source("false-success")
            }
        )
        .is_err()
    );
    p.apply(
        &state,
        EffectEvent::Reconcile {
            effect: EffectId::new("one").unwrap(),
            scope: scope(&state, "A"),
            at: 13,
            resolution: Reconciliation::Observed {
                outcome: EffectOutcome::Failed,
                usage: demand(40),
            },
            basis: source("measured-failure"),
        },
    )
    .unwrap();
    assert_eq!(p.effects()[0].report, Some(EffectOutcome::Failed));
    assert_eq!(p.accounted(Resource::WorkerSlots).unwrap(), 0);
    assert_eq!(p.accounted(Resource::ModelTokens).unwrap(), 40);
    assert!(
        p.apply(
            &state,
            EffectEvent::Reserve {
                intent: intent(&state, "D", "two"),
                at: 14
            }
        )
        .is_err()
    );
    let mut smaller = intent(&state, "D", "two");
    smaller.demand = demand(40);
    p.apply(
        &state,
        EffectEvent::Reserve {
            intent: smaller,
            at: 14,
        },
    )
    .unwrap();
}
#[test]
fn known_running_or_report_cannot_be_reconciled_as_absent() {
    let (state, mut p) = setup();
    reserve(&state, &mut p, "A", "one", 10);
    dispatch(&state, &mut p, "A", "one", 11);
    p.apply(
        &state,
        EffectEvent::Running {
            effect: EffectId::new("one").unwrap(),
            scope: scope(&state, "A"),
            at: 12,
            basis: source("running"),
        },
    )
    .unwrap();
    p.apply(
        &state,
        EffectEvent::Unknown {
            effect: EffectId::new("one").unwrap(),
            scope: scope(&state, "A"),
            at: 13,
            basis: source("lost-session"),
        },
    )
    .unwrap();
    let before = p.clone();
    assert_eq!(
        p.apply(
            &state,
            EffectEvent::Reconcile {
                effect: EffectId::new("one").unwrap(),
                scope: scope(&state, "A"),
                at: 14,
                resolution: Reconciliation::Absent,
                basis: source("unsafe-no-effect")
            }
        ),
        Err(ProgramError::Transition.into())
    );
    assert!(p == before);
}
#[test]
fn safe_retry_requires_exact_absence_and_cannot_fork_or_reset_chain() {
    let (state, mut p) = setup();
    reserve(&state, &mut p, "A", "one", 10);
    dispatch(&state, &mut p, "A", "one", 11);
    let mut retry = intent(&state, "A", "two");
    retry.retry_of = Some(EffectId::new("one").unwrap());
    assert!(
        p.apply(
            &state,
            EffectEvent::Reserve {
                intent: retry.clone(),
                at: 12
            }
        )
        .is_err()
    );
    absent(&state, &mut p, "A", "one", 12);
    assert!(
        p.apply(
            &state,
            EffectEvent::Reserve {
                intent: intent(&state, "A", "fresh-without-chain"),
                at: 13
            }
        )
        .is_err()
    );
    p.apply(
        &state,
        EffectEvent::Reserve {
            intent: retry,
            at: 13,
        },
    )
    .unwrap();
    dispatch(&state, &mut p, "A", "two", 14);
    absent(&state, &mut p, "A", "two", 15);
    let mut fork = intent(&state, "A", "fork");
    fork.retry_of = Some(EffectId::new("one").unwrap());
    assert!(
        p.apply(
            &state,
            EffectEvent::Reserve {
                intent: fork,
                at: 16
            }
        )
        .is_err()
    );
    let mut retry = intent(&state, "A", "three");
    retry.retry_of = Some(EffectId::new("two").unwrap());
    p.apply(
        &state,
        EffectEvent::Reserve {
            intent: retry,
            at: 16,
        },
    )
    .unwrap();
    dispatch(&state, &mut p, "A", "three", 17);
    absent(&state, &mut p, "A", "three", 18);
    let mut retry = intent(&state, "A", "four");
    retry.retry_of = Some(EffectId::new("three").unwrap());
    assert_eq!(
        p.apply(
            &state,
            EffectEvent::Reserve {
                intent: retry,
                at: 19
            }
        ),
        Err(ProgramError::Capacity.into())
    );
}
#[test]
fn new_policy_does_not_erase_pending_or_cumulative_resource_accounting() {
    let (state, mut p) = setup();
    reserve(&state, &mut p, "A", "one", 10);
    let mut changed = policy();
    changed.reference = source("new-policy");
    p.apply(&state, EffectEvent::Policy(changed.clone()))
        .unwrap();
    let mut next = operational_grant(&state, "D");
    next.id = GrantId::new("next-grant").unwrap();
    next.policy = changed.reference.clone();
    p.apply(&state, EffectEvent::Grant(next.clone())).unwrap();
    let mut next_intent = intent(&state, "D", "other");
    next_intent.grant = next.id;
    assert_eq!(
        p.apply(
            &state,
            EffectEvent::Reserve {
                intent: next_intent,
                at: 11
            }
        ),
        Err(OperationalError::ResourceLimit(Resource::WorkerSlots))
    );
    changed.reference = source("meaning-changed");
    changed.cumulative.clear();
    assert_eq!(
        p.apply(&state, EffectEvent::Policy(changed)),
        Err(ProgramError::Structure.into())
    );
    assert_eq!(p.accounted(Resource::WorkerSlots).unwrap(), 1);
}
#[test]
fn duplicate_missing_unknown_and_wrong_scope_inputs_refuse_atomically() {
    let (state, mut p) = setup();
    let mut bad = intent(&state, "A", "one");
    bad.demand.push(bad.demand[0].clone());
    let before = p.clone();
    assert!(
        p.apply(
            &state,
            EffectEvent::Reserve {
                intent: bad,
                at: 10
            }
        )
        .is_err()
    );
    assert!(p == before);
    let mut bad = intent(&state, "A", "one");
    bad.demand[1].amount = None;
    assert_eq!(
        p.apply(
            &state,
            EffectEvent::Reserve {
                intent: bad,
                at: 10
            }
        ),
        Err(OperationalError::UnknownResource(Resource::ModelTokens))
    );
    let mut bad = intent(&state, "A", "one");
    bad.scope.generation = rev(2);
    assert!(
        p.apply(
            &state,
            EffectEvent::Reserve {
                intent: bad,
                at: 10
            }
        )
        .is_err()
    );
    assert!(p == before);
    reserve(&state, &mut p, "A", "one", 10);
    let mut wrong = scope(&state, "A");
    wrong.candidate = CandidateRequirement::new(
        wrong.candidate.repository().clone(),
        candidate("9"),
        WorktreeRequirement::Clean,
    );
    assert!(
        p.apply(
            &state,
            EffectEvent::Dispatch {
                effect: EffectId::new("one").unwrap(),
                scope: wrong,
                at: 11,
                basis: source("wrong-candidate")
            }
        )
        .is_err()
    );
    assert_eq!(p.effects()[0].status, EffectStatus::Reserved);
}
#[test]
fn missing_preconditions_and_fan_in_stay_held_despite_metadata_claim() {
    let mut state = ProgramState::new(definition()).unwrap();
    state
        .claim(&grant(1, "operator"), attempt("attempt-A", "A", "operator"))
        .unwrap();
    let mut p = OperationalProjection::new();
    p.apply(&state, EffectEvent::Policy(policy())).unwrap();
    assert!(
        p.apply(&state, EffectEvent::Grant(operational_grant(&state, "A")))
            .is_err()
    );
    initialize(&mut state);
    p.apply(&state, EffectEvent::Grant(operational_grant(&state, "A")))
        .unwrap();
    state
        .claim(&grant(1, "operator"), attempt("attempt-F", "F", "operator"))
        .unwrap();
    assert!(
        p.apply(&state, EffectEvent::Grant(operational_grant(&state, "F")))
            .is_err()
    );
    reserve(&state, &mut p, "A", "allowed-neighbor", 10);
}
#[test]
fn complete_event_reconstruction_preserves_unknowns_and_original_failures() {
    let (state, mut p) = setup();
    reserve(&state, &mut p, "A", "one", 10);
    dispatch(&state, &mut p, "A", "one", 11);
    p.apply(
        &state,
        EffectEvent::Unknown {
            effect: EffectId::new("one").unwrap(),
            scope: scope(&state, "A"),
            at: 12,
            basis: source("interrupt"),
        },
    )
    .unwrap();
    let mut replay = OperationalProjection::new();
    for e in p.events() {
        replay.apply(&state, e.clone()).unwrap();
    }
    assert!(replay == p);
    assert_eq!(replay.effects()[0].status, EffectStatus::Unknown);
    assert_eq!(replay.accounted(Resource::WorkerSlots).unwrap(), 1);
}

#[test]
fn advisory_free_slots_do_not_block_current_owner_with_explicit_fixture_budget() {
    let (mut state, mut p) = setup();
    state
        .notify(
            &grant(1, "operator"),
            notify(
                &state,
                "no-more-mission-slots",
                Event::Capacity {
                    available: 0,
                    source: source("all-metadata-slots-claimed"),
                },
            ),
        )
        .unwrap();
    assert!(
        state
            .readiness()
            .task(&task("A"))
            .unwrap()
            .blockers()
            .iter()
            .any(|b| matches!(b, TaskBlocker::NoCapacity))
    );
    reserve(&state, &mut p, "A", "current-owner", 10);
    dispatch(&state, &mut p, "A", "current-owner", 11);
    // A distinct owner still cannot consume capacity reserved for verification.
    assert_eq!(
        p.apply(
            &state,
            EffectEvent::Reserve {
                intent: intent(&state, "D", "other"),
                at: 12
            }
        ),
        Err(OperationalError::ResourceLimit(Resource::WorkerSlots))
    );
}
#[test]
fn changed_package_or_grant_cannot_reset_retry_identity_within_one_attempt() {
    let (state, mut p) = setup();
    reserve(&state, &mut p, "A", "one", 10);
    p.apply(
        &state,
        EffectEvent::Cancel {
            effect: EffectId::new("one").unwrap(),
            scope: scope(&state, "A"),
            at: 11,
            basis: source("cancel-before-dispatch"),
        },
    )
    .unwrap();
    let mut changed = operational_grant(&state, "A");
    changed.id = GrantId::new("changed-package-grant").unwrap();
    changed.scope.package = PackageId::new("changed-package").unwrap();
    p.apply(&state, EffectEvent::Grant(changed.clone()))
        .unwrap();
    let mut fresh = intent(&state, "A", "fresh");
    fresh.grant = changed.id;
    fresh.scope = changed.scope;
    assert_eq!(
        p.apply(
            &state,
            EffectEvent::Reserve {
                intent: fresh,
                at: 12
            }
        ),
        Err(ProgramError::IdentityConflict.into())
    );
}
#[test]
fn replacing_policy_cannot_reset_effect_count_or_settled_cumulative_usage() {
    let (state, mut p) = setup();
    reserve(&state, &mut p, "A", "one", 10);
    dispatch(&state, &mut p, "A", "one", 11);
    p.apply(
        &state,
        EffectEvent::Reconcile {
            effect: EffectId::new("one").unwrap(),
            scope: scope(&state, "A"),
            at: 12,
            resolution: Reconciliation::Observed {
                outcome: EffectOutcome::Failed,
                usage: demand(40),
            },
            basis: source("settled-usage"),
        },
    )
    .unwrap();
    let mut next = policy();
    next.reference = source("next-policy");
    p.apply(&state, EffectEvent::Policy(next.clone())).unwrap();
    let mut g = operational_grant(&state, "D");
    g.id = GrantId::new("next-D").unwrap();
    g.policy = next.reference.clone();
    p.apply(&state, EffectEvent::Grant(g.clone())).unwrap();
    let mut d = intent(&state, "D", "second");
    d.grant = g.id;
    assert_eq!(
        p.apply(
            &state,
            EffectEvent::Reserve {
                intent: d.clone(),
                at: 13
            }
        ),
        Err(OperationalError::ResourceLimit(Resource::ModelTokens))
    );
    next.reference = source("bounded-policy");
    next.max_effects = 1;
    next.max_retries = 0;
    p.apply(&state, EffectEvent::Policy(next.clone())).unwrap();
    let mut g = operational_grant(&state, "D");
    g.id = GrantId::new("bounded-D").unwrap();
    g.policy = next.reference;
    p.apply(&state, EffectEvent::Grant(g.clone())).unwrap();
    d.grant = g.id;
    assert_eq!(
        p.apply(&state, EffectEvent::Reserve { intent: d, at: 13 }),
        Err(ProgramError::Capacity.into())
    );
    assert_eq!(p.accounted(Resource::ModelTokens).unwrap(), 40);
}
#[test]
fn reservation_overflow_and_unknown_reconciliation_usage_preserve_original_state() {
    let (state, _) = setup();
    let mut p = OperationalProjection::new();
    let mut selected = policy();
    selected.capacities[0].available = Some(3);
    selected.capacities[1].available = Some(u64::MAX);
    selected.capacities[1].verification_reserve = 0;
    p.apply(&state, EffectEvent::Policy(selected)).unwrap();
    for task in ["A", "D"] {
        let mut g = operational_grant(&state, task);
        g.ceilings = demand(u64::MAX);
        p.apply(&state, EffectEvent::Grant(g)).unwrap();
    }
    let mut first = intent(&state, "A", "one");
    first.demand = demand(u64::MAX);
    p.apply(
        &state,
        EffectEvent::Reserve {
            intent: first,
            at: 10,
        },
    )
    .unwrap();
    let mut second = intent(&state, "D", "two");
    second.demand = demand(1);
    let before = p.clone();
    assert_eq!(
        p.apply(
            &state,
            EffectEvent::Reserve {
                intent: second,
                at: 11
            }
        ),
        Err(OperationalError::ResourceLimit(Resource::ModelTokens))
    );
    assert!(p == before);
    dispatch(&state, &mut p, "A", "one", 11);
    let mut usage = demand(1);
    usage[1].amount = None;
    let before = p.clone();
    assert_eq!(
        p.apply(
            &state,
            EffectEvent::Reconcile {
                effect: EffectId::new("one").unwrap(),
                scope: scope(&state, "A"),
                at: 12,
                resolution: Reconciliation::Observed {
                    outcome: EffectOutcome::Failed,
                    usage
                },
                basis: source("missing-cost")
            }
        ),
        Err(OperationalError::UnknownResource(Resource::ModelTokens))
    );
    assert!(p == before);
}

#[test]
fn running_observation_after_cancel_request_is_retained_without_claiming_termination() {
    let (state, mut p) = setup();
    reserve(&state, &mut p, "A", "one", 10);
    dispatch(&state, &mut p, "A", "one", 11);
    p.apply(
        &state,
        EffectEvent::Cancel {
            effect: EffectId::new("one").unwrap(),
            scope: scope(&state, "A"),
            at: 12,
            basis: source("stop-request"),
        },
    )
    .unwrap();
    p.apply(
        &state,
        EffectEvent::Running {
            effect: EffectId::new("one").unwrap(),
            scope: scope(&state, "A"),
            at: 13,
            basis: source("still-running"),
        },
    )
    .unwrap();
    assert!(p.effects()[0].observed_running);
    assert_eq!(p.effects()[0].status, EffectStatus::CancellationRequested);
    assert!(
        p.apply(
            &state,
            EffectEvent::Reconcile {
                effect: EffectId::new("one").unwrap(),
                scope: scope(&state, "A"),
                at: 14,
                resolution: Reconciliation::Absent,
                basis: source("incorrect-absence")
            }
        )
        .is_err()
    );
    assert_eq!(p.accounted(Resource::WorkerSlots).unwrap(), 1);
}

fn preview_setup() -> (ProgramState, OperationalProjection) {
    let mut state = ProgramState::new(definition()).unwrap();
    initialize(&mut state);
    state
        .claim(&grant(1, "operator"), attempt("attempt-A", "A", "operator"))
        .unwrap();
    let mut p = OperationalProjection::new();
    p.apply(&state, EffectEvent::Policy(policy())).unwrap();
    p.apply(&state, EffectEvent::Grant(operational_grant(&state, "A")))
        .unwrap();
    (state, p)
}
fn proposal(tokens: u64) -> Proposal {
    Proposal {
        task: task("D"),
        band: 0,
        waiting_rounds: 0,
        risk_rank: 0,
        cost_rank: 0,
        demand: demand(tokens),
    }
}
#[test]
fn scheduler_preview_accounts_unknown_reservations_without_mutating_or_reserving() {
    let (state, mut p) = preview_setup();
    assert!(p.preview_schedule(&state, &[proposal(50)]).unwrap()[0].proposed);
    reserve(&state, &mut p, "A", "one", 10);
    dispatch(&state, &mut p, "A", "one", 11);
    p.apply(
        &state,
        EffectEvent::Unknown {
            effect: EffectId::new("one").unwrap(),
            scope: scope(&state, "A"),
            at: 12,
            basis: source("lost-reply"),
        },
    )
    .unwrap();
    let before = p.clone();
    let result = p.preview_schedule(&state, &[proposal(50)]).unwrap();
    assert!(result[0].eligible);
    assert!(!result[0].proposed);
    assert!(
        result[0]
            .held
            .contains(&WaitReason::Contention(Resource::WorkerSlots))
    );
    assert!(
        result[0]
            .held
            .contains(&WaitReason::Contention(Resource::ModelTokens))
    );
    assert!(p == before);
}
#[test]
fn scheduler_preview_releases_reusable_quantity_but_preserves_consumption_and_reserve() {
    let (state, mut p) = preview_setup();
    reserve(&state, &mut p, "A", "one", 10);
    dispatch(&state, &mut p, "A", "one", 11);
    p.apply(
        &state,
        EffectEvent::Reconcile {
            effect: EffectId::new("one").unwrap(),
            scope: scope(&state, "A"),
            at: 12,
            resolution: Reconciliation::Observed {
                outcome: EffectOutcome::Failed,
                usage: demand(40),
            },
            basis: source("settled-failure"),
        },
    )
    .unwrap();
    let too_much = p.preview_schedule(&state, &[proposal(50)]).unwrap();
    assert_eq!(
        too_much[0].held,
        vec![WaitReason::Contention(Resource::ModelTokens)]
    );
    let fits = p.preview_schedule(&state, &[proposal(40)]).unwrap();
    assert!(fits[0].proposed);
    assert!(fits[0].held.is_empty());
    assert_eq!(p.accounted(Resource::ModelTokens).unwrap(), 40);
    assert_eq!(p.accounted(Resource::WorkerSlots).unwrap(), 0);
}
#[test]
fn scheduler_preview_rejects_missing_dimensions_and_stale_policy_without_hiding_unknown_cost() {
    let (mut state, p) = preview_setup();
    let mut missing = proposal(50);
    missing.demand.pop();
    assert!(matches!(
        p.preview_schedule(&state, &[missing]),
        Err(OperationalError::Program(ProgramError::Structure))
    ));
    let mut unknown = proposal(50);
    unknown.demand[1].amount = None;
    assert_eq!(
        p.preview_schedule(&state, &[unknown]).unwrap()[0].held,
        vec![WaitReason::Unknown(Resource::ModelTokens)]
    );
    state
        .amend(&grant(1, "operator"), definition_revision(2, false))
        .unwrap();
    assert!(matches!(
        p.preview_schedule(&state, &[proposal(50)]),
        Err(OperationalError::Program(ProgramError::Stale))
    ));
}
#[test]
fn scheduler_reports_known_reserve_shortfall_as_contention_and_preserves_zero_neighbor() {
    let mut state = ProgramState::new(definition()).unwrap();
    initialize(&mut state);
    let p = Proposal {
        task: task("D"),
        band: 0,
        waiting_rounds: 0,
        risk_rank: 0,
        cost_rank: 0,
        demand: vec![Demand {
            resource: Resource::WorkerSlots,
            amount: Some(0),
        }],
    };
    let mut cap = Capacity {
        resource: Resource::WorkerSlots,
        available: Some(1),
        verification_reserve: 1,
        declared_enforceable: true,
    };
    assert!(
        simulate(&state, std::slice::from_ref(&p), std::slice::from_ref(&cap)).unwrap()[0].proposed
    );
    cap.available = Some(0);
    let insufficient = simulate(&state, &[p], &[cap]).unwrap();
    assert!(!insufficient[0].proposed);
    assert_eq!(
        insufficient[0].held,
        vec![WaitReason::Contention(Resource::WorkerSlots)]
    );
}

#[test]
fn cancellation_intent_survives_unknown_running_report_and_reconstruction() {
    let (state, mut p) = setup();
    reserve(&state, &mut p, "A", "one", 10);
    dispatch(&state, &mut p, "A", "one", 11);
    p.apply(
        &state,
        EffectEvent::Cancel {
            effect: EffectId::new("one").unwrap(),
            scope: scope(&state, "A"),
            at: 12,
            basis: source("requested-cancellation"),
        },
    )
    .unwrap();
    p.apply(
        &state,
        EffectEvent::Unknown {
            effect: EffectId::new("one").unwrap(),
            scope: scope(&state, "A"),
            at: 13,
            basis: source("cleanup-unconfirmed"),
        },
    )
    .unwrap();
    assert_eq!(p.effects()[0].status, EffectStatus::Unknown);
    assert!(p.effects()[0].cancellation_requested);
    p.apply(
        &state,
        EffectEvent::Running {
            effect: EffectId::new("one").unwrap(),
            scope: scope(&state, "A"),
            at: 14,
            basis: source("still-running-after-unknown"),
        },
    )
    .unwrap();
    assert_eq!(p.effects()[0].status, EffectStatus::CancellationRequested);
    assert!(p.effects()[0].observed_running);
    assert!(p.effects()[0].cancellation_requested);
    assert!(
        p.apply(
            &state,
            EffectEvent::Reconcile {
                effect: EffectId::new("one").unwrap(),
                scope: scope(&state, "A"),
                at: 15,
                resolution: Reconciliation::Absent,
                basis: source("incorrect-absence"),
            }
        )
        .is_err()
    );
    assert_eq!(p.accounted(Resource::WorkerSlots).unwrap(), 1);
    p.apply(
        &state,
        EffectEvent::Report {
            effect: EffectId::new("one").unwrap(),
            scope: scope(&state, "A"),
            at: 15,
            outcome: EffectOutcome::Interrupted,
            basis: source("interrupted-report"),
        },
    )
    .unwrap();
    p.apply(
        &state,
        EffectEvent::Reconcile {
            effect: EffectId::new("one").unwrap(),
            scope: scope(&state, "A"),
            at: 16,
            resolution: Reconciliation::Observed {
                outcome: EffectOutcome::Interrupted,
                usage: demand(30),
            },
            basis: source("confirmed-interruption"),
        },
    )
    .unwrap();
    assert!(p.effects()[0].cancellation_requested);
    assert_eq!(p.effects()[0].status, EffectStatus::ReconciledReported);
    let mut replay = OperationalProjection::new();
    for event in p.events() {
        replay.apply(&state, event.clone()).unwrap();
    }
    assert!(replay == p);
    assert!(replay.effects()[0].cancellation_requested);
}

#[test]
fn generic_allocation_rejects_physical_observation_scopes_and_accepts_exact_accounting_pools() {
    use cortex_core::program::{domain::PoolIdentity, memory::*};
    let mut state = ProgramState::new(definition()).unwrap();
    initialize(&mut state);
    let process = MemoryProcessIdentity {
        host: "host".into(),
        boot: "boot".into(),
        pid: 1,
        start_ticks: 10,
    };
    let resources = [
        Resource::memory(
            MemoryBasis::ProcessResident,
            MemoryScope::Process(process.clone()),
        )
        .unwrap(),
        Resource::memory(
            MemoryBasis::SummedProcessResident,
            MemoryScope::ProcessSet(vec![process]),
        )
        .unwrap(),
        Resource::memory(
            MemoryBasis::CgroupCharged,
            MemoryScope::Cgroup {
                host: "host".into(),
                boot: "boot".into(),
                manager: "manager".into(),
                invocation: "invocation".into(),
                path: "/owned/scope".into(),
                device: 1,
                inode: 2,
            },
        )
        .unwrap(),
    ];
    let capacity = |resource| Capacity {
        resource,
        available: Some(1024),
        verification_reserve: 256,
        declared_enforceable: true,
    };
    let proposal = |resource| Proposal {
        task: task("A"),
        band: 0,
        waiting_rounds: 0,
        risk_rank: 0,
        cost_rank: 0,
        demand: vec![Demand {
            resource,
            amount: Some(512),
        }],
    };
    for resource in resources {
        let mut policy = policy();
        policy.capacities.push(capacity(resource.clone()));
        assert!(
            OperationalProjection::new()
                .apply(&state, EffectEvent::Policy(policy))
                .is_err()
        );
        assert!(
            simulate(
                &state,
                &[proposal(resource.clone())],
                &[capacity(resource.clone())]
            )
            .is_err()
        );
        assert!(simulate(&state, &[proposal(resource)], &[]).is_err());
    }
    for basis in [
        MemoryBasis::ProcessResident,
        MemoryBasis::SummedProcessResident,
        MemoryBasis::CgroupCharged,
    ] {
        let resource = Resource::memory(
            basis,
            MemoryScope::AccountingPool(PoolIdentity {
                physical: "pool".into(),
                incarnation: "allocation".into(),
                host: "host".into(),
                boot: "boot".into(),
            }),
        )
        .unwrap();
        let mut policy = policy();
        policy.capacities.push(capacity(resource.clone()));
        OperationalProjection::new()
            .apply(&state, EffectEvent::Policy(policy))
            .unwrap();
        let decisions =
            simulate(&state, &[proposal(resource.clone())], &[capacity(resource)]).unwrap();
        assert!(decisions[0].proposed);
    }
    let unknown = Resource::UnspecifiedMemoryBytes;
    let held = simulate(
        &state,
        &[proposal(unknown.clone())],
        &[capacity(unknown.clone())],
    )
    .unwrap();
    assert!(!held[0].proposed);
    assert!(held[0].held.contains(&WaitReason::Unknown(unknown)));
}
