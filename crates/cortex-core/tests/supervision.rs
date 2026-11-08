//! Pure supervised-profile guards; these tests never launch a process.
#[path = "support/program.rs"]
mod fixture;
#[path = "support/supervision.rs"]
mod support;
use cortex_core::{
    binding::ObservationId,
    program::{operational::*, scheduling::*, supervision::*, *},
};
use support::*;

#[test]
fn process_profile_is_explicit_and_cannot_be_selected_by_a_policy_label() {
    let (state, mut supervised, s) = setup();
    let mut pure = OperationalProjection::new();
    assert_eq!(
        pure.apply(&state, EffectEvent::Policy(policy())),
        Err(OperationalError::ExternalUnavailable)
    );
    let mut p = policy();
    p.mode = EffectMode::ControlledFixture;
    assert_eq!(
        supervised.apply(&state, EffectEvent::Policy(p)),
        Err(OperationalError::ExternalUnavailable)
    );
    let mut p = policy();
    p.mode = EffectMode::ExternalExecutor;
    assert_eq!(
        supervised.apply(&state, EffectEvent::Policy(p)),
        Err(OperationalError::ExternalUnavailable)
    );
    assert_eq!(
        pure.apply(
            &state,
            EffectEvent::PrepareProcess {
                effect: intent(&s).id,
                scope: s.clone(),
                at: 1,
                spec: Box::new(spec(&s))
            }
        ),
        Err(OperationalError::ExternalUnavailable)
    );
}
#[test]
fn immutable_preparation_must_precede_dispatch_with_exact_privacy_host_and_budget() {
    let (state, mut p, s) = setup();
    let before = p.clone();
    assert!(
        p.apply(
            &state,
            EffectEvent::Dispatch {
                effect: intent(&s).id,
                scope: s.clone(),
                at: 3,
                basis: source("too-early")
            }
        )
        .is_err()
    );
    assert!(p == before);
    let mut bad = spec(&s);
    bad.privacy = source("unapproved-disclosure");
    assert!(
        p.apply(
            &state,
            EffectEvent::PrepareProcess {
                effect: intent(&s).id,
                scope: s.clone(),
                at: 2,
                spec: Box::new(bad)
            }
        )
        .is_err()
    );
    let mut bad = spec(&s);
    bad.limits.wall_ms = 1201;
    assert!(
        p.apply(
            &state,
            EffectEvent::PrepareProcess {
                effect: intent(&s).id,
                scope: s.clone(),
                at: 2,
                spec: Box::new(bad)
            }
        )
        .is_err()
    );
    assert!(p == before);
    prepare(&state, &mut p, &s);
    assert!(
        p.apply(
            &state,
            EffectEvent::PrepareProcess {
                effect: intent(&s).id,
                scope: s.clone(),
                at: 2,
                spec: Box::new(spec(&s))
            }
        )
        .is_err()
    );
    dispatch(&state, &mut p, &s);
    assert_eq!(p.effects()[0].dispatch_at, Some(3));
    assert_eq!(p.effects()[0].status, EffectStatus::DispatchUncertain);
}
#[test]
fn prelaunch_uncertainty_and_generic_reports_cannot_bypass_process_receipts() {
    let (state, mut p, s) = setup();
    prepare(&state, &mut p, &s);
    dispatch(&state, &mut p, &s);
    for e in [
        EffectEvent::Running {
            effect: intent(&s).id,
            scope: s.clone(),
            at: 4,
            basis: source("untyped-running"),
        },
        EffectEvent::Report {
            effect: intent(&s).id,
            scope: s.clone(),
            at: 4,
            outcome: EffectOutcome::Succeeded,
            basis: source("untyped-success"),
        },
        EffectEvent::Reconcile {
            effect: intent(&s).id,
            scope: s.clone(),
            at: 4,
            resolution: Reconciliation::Absent,
            basis: source("silent-is-not-absent"),
        },
    ] {
        let before = p.clone();
        assert!(p.apply(&state, e).is_err());
        assert!(p == before);
    }
    assert!(p.has_pending_attempt(&s.attempt));
    assert_eq!(p.accounted(Resource::WorkerSlots).unwrap(), 1);
}
#[test]
fn captured_incarnation_and_observation_identity_cannot_be_replayed_or_replaced() {
    let (state, mut p, s) = setup();
    prepare(&state, &mut p, &s);
    dispatch(&state, &mut p, &s);
    let spawned = observation(&s, 1, ProcessCompletion::Spawned, Quiescence::Unknown);
    observed(&state, &mut p, &s, spawned.clone(), 4);
    assert_eq!(p.effects()[0].status, EffectStatus::Running);
    let before = p.clone();
    assert!(
        p.apply(
            &state,
            EffectEvent::ProcessObserved {
                effect: intent(&s).id,
                scope: s.clone(),
                at: 5,
                observation: spawned
            }
        )
        .is_err()
    );
    assert!(p == before);
    let mut changed = observation(
        &s,
        2,
        ProcessCompletion::Exited {
            code: Some(0),
            signal: None,
        },
        Quiescence::DirectChildReapedStreamsClosed,
    );
    changed.process.as_mut().unwrap().start_ticks += 1;
    assert!(
        p.apply(
            &state,
            EffectEvent::ProcessObserved {
                effect: intent(&s).id,
                scope: s.clone(),
                at: 5,
                observation: changed
            }
        )
        .is_err()
    );
    assert!(p == before);
}
#[test]
fn cancellation_and_revocation_stop_authority_without_claiming_quiescence() {
    let (state, mut p, s) = setup();
    prepare(&state, &mut p, &s);
    dispatch(&state, &mut p, &s);
    let spawned = observation(&s, 1, ProcessCompletion::Spawned, Quiescence::Unknown);
    observed(&state, &mut p, &s, spawned, 4);
    p.check_supervised_current(&state, &intent(&s).id, &s, 4)
        .unwrap();
    p.apply(
        &state,
        EffectEvent::Revoke {
            grant: proposed_grant(&s).id,
            at: 5,
            basis: source("revoke"),
        },
    )
    .unwrap();
    assert_eq!(
        p.check_supervised_current(&state, &intent(&s).id, &s, 5),
        Err(OperationalError::GrantUnavailable)
    );
    p.apply(
        &state,
        EffectEvent::Cancel {
            effect: intent(&s).id,
            scope: s.clone(),
            at: 6,
            basis: source("cancel"),
        },
    )
    .unwrap();
    let unknown = observation(
        &s,
        2,
        ProcessCompletion::Interrupted {
            reason: InterruptionReason::Cancelled,
        },
        Quiescence::Unknown,
    );
    observed(&state, &mut p, &s, unknown.clone(), 7);
    assert!(
        p.apply(
            &state,
            EffectEvent::Reconcile {
                effect: intent(&s).id,
                scope: s.clone(),
                at: 8,
                resolution: Reconciliation::Observed {
                    outcome: EffectOutcome::Interrupted,
                    usage: usage(&unknown)
                },
                basis: source("no-cleanup")
            }
        )
        .is_err()
    );
    assert_eq!(p.accounted(Resource::WorkerSlots).unwrap(), 1);
    let mut done = unknown.clone();
    done.id = ObservationId::new("process-fixture", 3).unwrap();
    done.quiescence = Quiescence::DirectChildReapedStreamsClosed;
    done.elapsed_ms = 3;
    observed(&state, &mut p, &s, done.clone(), 8);
    p.apply(
        &state,
        EffectEvent::Reconcile {
            effect: intent(&s).id,
            scope: s.clone(),
            at: 9,
            resolution: Reconciliation::Observed {
                outcome: EffectOutcome::Interrupted,
                usage: usage(&done),
            },
            basis: source("captured-cleanup"),
        },
    )
    .unwrap();
    assert!(!p.has_pending_attempt(&s.attempt));
    assert!(p.effects()[0].cancellation_requested);
    assert_eq!(p.effects()[0].process_observations.len(), 3);
}
#[test]
fn positive_started_observation_prevents_false_absence_but_prespawn_refusal_allows_it() {
    let (state, mut p, s) = setup();
    prepare(&state, &mut p, &s);
    dispatch(&state, &mut p, &s);
    let absent = observation(
        &s,
        1,
        ProcessCompletion::StartFailed,
        Quiescence::NoChildStarted,
    );
    observed(&state, &mut p, &s, absent, 4);
    p.apply(
        &state,
        EffectEvent::Reconcile {
            effect: intent(&s).id,
            scope: s.clone(),
            at: 5,
            resolution: Reconciliation::Absent,
            basis: source("explicit-prespawn-refusal"),
        },
    )
    .unwrap();
    assert_eq!(p.effects()[0].status, EffectStatus::ReconciledAbsent);
    let (state, mut p, s) = setup();
    prepare(&state, &mut p, &s);
    dispatch(&state, &mut p, &s);
    observed(
        &state,
        &mut p,
        &s,
        observation(&s, 1, ProcessCompletion::Spawned, Quiescence::Unknown),
        4,
    );
    assert!(
        p.apply(
            &state,
            EffectEvent::ProcessObserved {
                effect: intent(&s).id,
                scope: s.clone(),
                at: 5,
                observation: observation(
                    &s,
                    2,
                    ProcessCompletion::StartFailed,
                    Quiescence::NoChildStarted
                )
            }
        )
        .is_err()
    );
}
#[test]
fn settlement_requires_exact_captured_usage_and_preserves_historical_scope() {
    let (mut state, mut p, s) = setup();
    prepare(&state, &mut p, &s);
    dispatch(&state, &mut p, &s);
    state
        .amend(&grant(1, "operator"), definition_revision(2, true))
        .unwrap();
    assert!(
        p.check_supervised_current(&state, &intent(&s).id, &s, 4)
            .is_err()
    );
    let done = observation(
        &s,
        1,
        ProcessCompletion::Exited {
            code: Some(0),
            signal: None,
        },
        Quiescence::DirectChildReapedStreamsClosed,
    );
    observed(&state, &mut p, &s, done.clone(), 4);
    let mut wrong = usage(&done);
    wrong[1].amount = Some(0);
    assert!(
        p.apply(
            &state,
            EffectEvent::Reconcile {
                effect: intent(&s).id,
                scope: s.clone(),
                at: 5,
                resolution: Reconciliation::Observed {
                    outcome: EffectOutcome::Succeeded,
                    usage: wrong
                },
                basis: source("wrong-accounting")
            }
        )
        .is_err()
    );
    p.apply(
        &state,
        EffectEvent::Reconcile {
            effect: intent(&s).id,
            scope: s.clone(),
            at: 5,
            resolution: Reconciliation::Observed {
                outcome: EffectOutcome::Succeeded,
                usage: usage(&done),
            },
            basis: source("historical-exact-outcome"),
        },
    )
    .unwrap();
    assert_eq!(state.attempts()[0].status, AttemptStatus::Claimed);
}
#[test]
fn exit_zero_cannot_hide_incomplete_input_malformed_output_or_unobserved_cleanup() {
    let (state, mut p, s) = setup();
    prepare(&state, &mut p, &s);
    dispatch(&state, &mut p, &s);
    let mut done = observation(
        &s,
        1,
        ProcessCompletion::Exited {
            code: Some(0),
            signal: None,
        },
        Quiescence::Unknown,
    );
    done.input_bytes = 9;
    done.malformed_output = true;
    observed(&state, &mut p, &s, done.clone(), 4);
    assert_eq!(p.effects()[0].report, Some(EffectOutcome::Failed));
    assert_eq!(p.effects()[0].status, EffectStatus::Unknown);
    assert!(
        p.apply(
            &state,
            EffectEvent::Reconcile {
                effect: intent(&s).id,
                scope: s.clone(),
                at: 5,
                resolution: Reconciliation::Observed {
                    outcome: EffectOutcome::Failed,
                    usage: usage(&done)
                },
                basis: source("exit-not-cleanup")
            }
        )
        .is_err()
    );
}
#[test]
fn finite_process_shape_rejects_relative_paths_null_arguments_and_unbounded_limits() {
    let (state, _, s) = setup();
    let valid = spec(&s);
    valid.validate(state.definition().limits()).unwrap();
    for change in 0..5 {
        let mut bad = valid.clone();
        match change {
            0 => bad.executable = "relative".into(),
            1 => bad.args = vec!["nul\0argument".into()],
            2 => bad.limits.wall_ms = 0,
            3 => bad.limits.frame_bytes = 1001,
            _ => bad.limits.executable_bytes = 0,
        };
        assert!(bad.validate(state.definition().limits()).is_err());
    }
}

#[test]
fn optional_transport_has_explicit_finite_limits_without_changing_legacy_shape() {
    let (state, _, scope) = setup();
    let mut s = spec(&scope);
    assert!(s.limits.transport.is_none());
    s.limits.transport = Some(TransportLimits {
        startup_ms: 200,
        idle_ms: 100,
        stdout_bytes: 900,
        stderr_bytes: 100,
        records: 8,
        buffer_bytes: 8302, // exact input + frame + read scratch
    });
    s.validate(state.definition().limits()).unwrap();
    for change in 0..9 {
        let mut bad = s.clone();
        let t = bad.limits.transport.as_mut().unwrap();
        match change {
            0 => t.startup_ms = 0,
            1 => t.startup_ms = 1001,
            2 => t.idle_ms = 0,
            3 => t.idle_ms = 1001,
            4 => t.stdout_bytes = 0,
            5 => t.stdout_bytes = 1001,
            6 => t.stderr_bytes = 1001,
            7 => t.records = 0,
            _ => t.buffer_bytes = 8301,
        }
        assert!(bad.validate(state.definition().limits()).is_err());
    }
    s.limits.transport.as_mut().unwrap().stderr_bytes = 0;
    s.validate(state.definition().limits()).unwrap();
}

#[test]
fn supplied_exit_zero_does_not_override_transport_count_or_startup_obligations() {
    let (_, _, scope) = setup();
    let mut s = spec(&scope);
    s.limits.transport = Some(TransportLimits {
        startup_ms: 200,
        idle_ms: 100,
        stdout_bytes: 3,
        stderr_bytes: 0,
        records: 1,
        buffer_bytes: 8302,
    });
    let valid = observation(
        &scope,
        1,
        ProcessCompletion::Exited {
            code: Some(0),
            signal: None,
        },
        Quiescence::DirectChildReapedStreamsClosed,
    );
    assert_eq!(valid.outcome(&s), Some(EffectOutcome::Succeeded));
    for change in 0..4 {
        let mut false_success = valid.clone();
        match change {
            0 => false_success.stdout_bytes = 4,
            1 => false_success.stderr_bytes = 1,
            2 => false_success.structured_records = 0,
            _ => false_success.structured_records = 2,
        }
        assert_eq!(false_success.outcome(&s), Some(EffectOutcome::Failed));
    }
}
#[test]
fn supervised_replay_retains_process_identity_without_creating_a_launch_or_clock_origin() {
    let (state, mut p, s) = setup();
    prepare(&state, &mut p, &s);
    dispatch(&state, &mut p, &s);
    observed(
        &state,
        &mut p,
        &s,
        observation(&s, 1, ProcessCompletion::Spawned, Quiescence::Unknown),
        4,
    );
    p.apply(
        &state,
        EffectEvent::Unknown {
            effect: intent(&s).id,
            scope: s.clone(),
            at: 5,
            basis: source("controller-interrupted"),
        },
    )
    .unwrap();
    let mut replay = OperationalProjection::new_supervised_fixture();
    for e in p.events() {
        replay.apply(&state, e.clone()).unwrap();
    }
    assert!(replay == p);
    assert_eq!(replay.effects()[0].dispatch_at, Some(3));
    assert_eq!(replay.effects()[0].status, EffectStatus::Unknown);
    assert!(
        replay
            .check_supervised_current(&state, &intent(&s).id, &s, 6)
            .is_err()
    );
    assert_eq!(
        replay.effects()[0].process_observations[0]
            .process
            .as_ref()
            .unwrap()
            .start_ticks,
        456
    );
}

#[test]
fn measured_prespawn_refusal_keeps_actual_controller_cost_without_fictional_child_use() {
    let (state, mut p, s) = setup();
    prepare(&state, &mut p, &s);
    dispatch(&state, &mut p, &s);
    let mut refused = observation(
        &s,
        1,
        ProcessCompletion::StartFailed,
        Quiescence::NoChildStarted,
    );
    refused.elapsed_ms = 7000;
    observed(&state, &mut p, &s, refused, 7003);
    assert_eq!(p.accounted(Resource::ElapsedMillis).unwrap(), 7000);
    p.apply(
        &state,
        EffectEvent::Reconcile {
            effect: intent(&s).id,
            scope: s.clone(),
            at: 7004,
            resolution: Reconciliation::Absent,
            basis: source("measured-prespawn-refusal"),
        },
    )
    .unwrap();
    assert_eq!(p.accounted(Resource::ElapsedMillis).unwrap(), 7000);
    assert_eq!(p.accounted(Resource::WorkerSlots).unwrap(), 0);
    assert_eq!(p.accounted(Resource::OutputBytes).unwrap(), 0);
    let waiting = Proposal {
        task: task("D"),
        band: 0,
        waiting_rounds: 0,
        risk_rank: 0,
        cost_rank: 0,
        demand: demand(),
    };
    let decision = p.preview_schedule(&state, &[waiting]).unwrap();
    assert!(
        decision[0]
            .held
            .contains(&WaitReason::Contention(Resource::ElapsedMillis))
    );
}
#[test]
fn supervised_overrun_is_charged_while_pending_and_after_quiescent_failure() {
    let (state, mut p, s) = setup();
    prepare(&state, &mut p, &s);
    dispatch(&state, &mut p, &s);
    let mut exceeded = observation(
        &s,
        1,
        ProcessCompletion::Exited {
            code: Some(0),
            signal: None,
        },
        Quiescence::Unknown,
    );
    exceeded.elapsed_ms = 6000;
    exceeded.stdout_bytes = 6000;
    observed(&state, &mut p, &s, exceeded.clone(), 6003);
    assert_eq!(p.effects()[0].report, Some(EffectOutcome::Failed));
    assert_eq!(p.accounted(Resource::ElapsedMillis).unwrap(), 6000);
    assert_eq!(p.accounted(Resource::OutputBytes).unwrap(), 6000);
    let mut lower = exceeded.clone();
    lower.id = ObservationId::new("process-fixture", 2).unwrap();
    lower.stdout_bytes = 3;
    lower.elapsed_ms = 2;
    lower.quiescence = Quiescence::DirectChildReapedStreamsClosed;
    assert!(
        p.apply(
            &state,
            EffectEvent::ProcessObserved {
                effect: intent(&s).id,
                scope: s.clone(),
                at: 6004,
                observation: lower
            }
        )
        .is_err()
    );
    let mut done = exceeded;
    done.id = ObservationId::new("process-fixture", 2).unwrap();
    done.quiescence = Quiescence::DirectChildReapedStreamsClosed;
    observed(&state, &mut p, &s, done.clone(), 6004);
    p.apply(
        &state,
        EffectEvent::Reconcile {
            effect: intent(&s).id,
            scope: s.clone(),
            at: 6005,
            resolution: Reconciliation::Observed {
                outcome: EffectOutcome::Failed,
                usage: usage(&done),
            },
            basis: source("observed-overrun-is-not-clamped"),
        },
    )
    .unwrap();
    assert_eq!(p.accounted(Resource::WorkerSlots).unwrap(), 0);
    assert_eq!(p.accounted(Resource::ElapsedMillis).unwrap(), 6000);
    assert_eq!(p.accounted(Resource::OutputBytes).unwrap(), 6000);
    let waiting = Proposal {
        task: task("D"),
        band: 0,
        waiting_rounds: 0,
        risk_rank: 0,
        cost_rank: 0,
        demand: demand(),
    };
    assert!(!p.preview_schedule(&state, &[waiting]).unwrap()[0].proposed);
}
#[test]
fn early_exit_zero_is_not_terminal_success_before_cleanup_and_final_accounting() {
    let (state, mut p, s) = setup();
    prepare(&state, &mut p, &s);
    dispatch(&state, &mut p, &s);
    let early = observation(
        &s,
        1,
        ProcessCompletion::Exited {
            code: Some(0),
            signal: None,
        },
        Quiescence::Unknown,
    );
    observed(&state, &mut p, &s, early.clone(), 4);
    assert_eq!(p.effects()[0].report, None);
    let mut late = early;
    late.id = ObservationId::new("process-fixture", 2).unwrap();
    late.elapsed_ms = 1500;
    late.quiescence = Quiescence::DirectChildReapedStreamsClosed;
    observed(&state, &mut p, &s, late, 1503);
    assert_eq!(p.effects()[0].report, Some(EffectOutcome::Failed));
}

#[test]
fn readonly_authority_check_cannot_reuse_a_historical_tick_after_time_advanced() {
    let (state, mut p, s) = setup();
    prepare(&state, &mut p, &s);
    dispatch(&state, &mut p, &s);
    p.check_supervised_current(&state, &intent(&s).id, &s, 3)
        .unwrap();
    let mut unrelated = proposed_grant(&s);
    unrelated.id = GrantId::new("unrelated-grant").unwrap();
    p.apply(&state, EffectEvent::Grant(unrelated.clone()))
        .unwrap();
    p.apply(
        &state,
        EffectEvent::Revoke {
            grant: unrelated.id,
            at: 5000,
            basis: source("later-controller-tick"),
        },
    )
    .unwrap();
    assert_eq!(
        p.check_supervised_current(&state, &intent(&s).id, &s, 3),
        Err(OperationalError::ClockRegression)
    );
    assert_eq!(
        p.check_supervised_current(&state, &intent(&s).id, &s, 5000),
        Err(OperationalError::GrantUnavailable)
    );
    assert!(p.has_pending_attempt(&s.attempt));
}

#[test]
fn owned_scope_requires_exact_incarnation_controls_membership_and_whole_set_cleanup() {
    let (_, _, scope) = setup();
    let spec = contained_spec(&scope);
    let expected = spec.containment.as_ref().unwrap();
    let mut observed = observation(
        &scope,
        1,
        ProcessCompletion::Exited {
            code: Some(0),
            signal: None,
        },
        Quiescence::OwnedScopeEmpty,
    );
    observed.containment = Some(Box::new(scope_observation(expected, Some(false))));
    assert!(observed.validate(&spec).is_ok());
    assert_eq!(observed.outcome(&spec), Some(EffectOutcome::Succeeded));
    for case in 0..9 {
        let mut bad = observed.clone();
        let facts = bad.containment.as_mut().unwrap();
        match case {
            0 => facts.populated = Some(true),
            1 => facts.populated = None,
            2 => facts.controls = None,
            3 => facts.membership_verified = false,
            4 => facts.helper_quiescent = false,
            5 => facts.lifecycle_verified = false,
            6 => facts.identity.invocation_id = "00000000000000000000000000000000".into(),
            7 => facts.identity.unit.push_str("-foreign"),
            _ => bad.quiescence = Quiescence::DirectChildReapedStreamsClosed,
        }
        assert!(bad.validate(&spec).is_err(), "case {case}");
        assert_ne!(bad.outcome(&spec), Some(EffectOutcome::Succeeded));
    }
    observed.containment.as_mut().unwrap().memory_peak_bytes = None;
    assert_eq!(observed.outcome(&spec), None);
}

#[test]
fn unknown_controller_helper_cannot_certify_absence_or_cleanup() {
    let (_, _, scope) = setup();
    let spec = contained_spec(&scope);
    let mut observed = observation(&scope, 1, ProcessCompletion::Unknown, Quiescence::Unknown);
    observed.process = None;
    observed.unresolved_scope_helpers = vec![ScopeHelperIdentity {
        pid: 777,
        start_ticks: None,
    }];
    assert!(observed.validate(&spec).is_ok());
    assert!(!observed.quiescent(&spec));
    assert_eq!(observed.outcome(&spec), None);
    observed.completion = ProcessCompletion::StartFailed;
    observed.quiescence = Quiescence::NoChildStarted;
    assert!(observed.validate(&spec).is_err());
    observed.unresolved_scope_helpers.clear();
    assert!(observed.validate(&spec).is_ok());
}

#[test]
fn owned_scope_resource_contract_separates_rate_from_cumulative_time() {
    let (_, _, scope) = setup();
    let mut spec = contained_spec(&scope);
    let limits = definition().limits();
    assert!(spec.validate(limits).is_ok());
    let containment = spec.containment.as_mut().unwrap();
    assert_eq!(containment.cpu_capacity_millicores(), Some(1000));
    assert_eq!(containment.elapsed_envelope_ms(&spec.limits), Some(7000));
    containment.cpu_time_ms = 1500;
    assert_eq!(containment.cpu_capacity_millicores(), Some(1000));
    containment.controls.cpu_quota_us = 50_000;
    assert_eq!(containment.cpu_capacity_millicores(), Some(500));
    containment.escalation_ms = u64::MAX;
    assert!(spec.validate(limits).is_err());
}

#[test]
fn cpu_millicore_accessor_preserves_rounding_zero_and_overflow_boundaries() {
    let (_, _, scope) = setup();
    let mut spec = contained_spec(&scope);
    let limits = definition().limits();
    let controls = &mut spec.containment.as_mut().unwrap().controls;
    controls.cpu_quota_us = 1;
    controls.cpu_period_us = 100_000;
    assert_eq!(
        spec.containment.as_ref().unwrap().cpu_capacity_millicores(),
        Some(1)
    );
    spec.containment.as_mut().unwrap().controls.cpu_quota_us = 0;
    assert_eq!(
        spec.containment.as_ref().unwrap().cpu_capacity_millicores(),
        Some(0)
    );
    // Arithmetic observation and valid admission are deliberately distinct.
    assert!(spec.validate(limits).is_err());
    spec.containment.as_mut().unwrap().controls.cpu_period_us = 0;
    assert_eq!(
        spec.containment.as_ref().unwrap().cpu_capacity_millicores(),
        None
    );
    spec.containment.as_mut().unwrap().controls.cpu_period_us = 100_000;
    spec.containment.as_mut().unwrap().controls.cpu_quota_us = u64::MAX;
    assert_eq!(
        spec.containment.as_ref().unwrap().cpu_capacity_millicores(),
        None
    );
}

fn checkpoint_setup() -> (
    ProgramState,
    OperationalProjection,
    EffectScope,
    ProcessSpec,
) {
    let (state, _, scope) = setup();
    let mut spec = contained_spec(&scope);
    spec.identity_checkpoint = Some(IdentityCheckpointPolicy::BeforeInputV1);
    let mut projection = OperationalProjection::new_supervised_fixture();
    for event in [
        EffectEvent::Policy(scope_policy()),
        EffectEvent::Grant(scope_grant(&scope)),
        EffectEvent::Reserve {
            intent: scope_intent(&scope),
            at: 1,
        },
        EffectEvent::PrepareProcess {
            effect: intent(&scope).id,
            scope: scope.clone(),
            at: 2,
            spec: Box::new(spec.clone()),
        },
        EffectEvent::Dispatch {
            effect: intent(&scope).id,
            scope: scope.clone(),
            at: 3,
            basis: source("checkpoint-dispatch"),
        },
    ] {
        projection.apply(&state, event).unwrap();
    }
    (state, projection, scope, spec)
}
fn checkpoint_capture(scope: &EffectScope, spec: &ProcessSpec, n: u64) -> ProcessObservation {
    let mut captured = observation(
        scope,
        n,
        ProcessCompletion::IdentityVerified,
        Quiescence::Unknown,
    );
    captured.containment = Some(Box::new(scope_observation(
        spec.containment.as_ref().unwrap(),
        Some(true),
    )));
    captured
}
fn captured_event(scope: &EffectScope, at: u64, observation: ProcessObservation) -> EffectEvent {
    EffectEvent::ProcessObserved {
        effect: intent(scope).id,
        scope: scope.clone(),
        at,
        observation,
    }
}
#[test]
fn identity_checkpoint_requires_an_explicit_complete_nonterminal_preinput_contract() {
    let (_, _, scope, mut spec) = checkpoint_setup();
    let good = checkpoint_capture(&scope, &spec, 2);
    assert!(good.validate(&spec).is_ok());
    assert_eq!(good.outcome(&spec), None);
    assert!(!good.quiescent(&spec));
    for case in 0..8 {
        let mut bad = good.clone();
        match case {
            0 => bad.input_bytes = 1,
            1 => bad.process = None,
            2 => bad.containment.as_mut().unwrap().populated = None,
            3 => bad.containment.as_mut().unwrap().populated = Some(false),
            4 => bad.containment.as_mut().unwrap().membership_verified = false,
            5 => bad.containment.as_mut().unwrap().lifecycle_verified = false,
            6 => bad.unresolved_scope_helpers.push(ScopeHelperIdentity {
                pid: 777,
                start_ticks: None,
            }),
            _ => bad.quiescence = Quiescence::OwnedScopeEmpty,
        }
        assert!(bad.validate(&spec).is_err(), "case {case}");
    }
    spec.identity_checkpoint = None;
    assert!(good.validate(&spec).is_err());
    spec.identity_checkpoint = Some(IdentityCheckpointPolicy::BeforeInputV1);
    spec.containment = None;
    assert!(spec.validate(definition().limits()).is_err());
}
#[test]
fn checkpoint_is_once_after_spawn_and_before_any_input_with_atomic_refusals() {
    let (state, mut projection, scope, spec) = checkpoint_setup();
    let before = projection.clone();
    assert!(
        projection
            .apply(
                &state,
                captured_event(&scope, 4, checkpoint_capture(&scope, &spec, 2))
            )
            .is_err()
    );
    assert!(projection == before);
    let spawned = observation(&scope, 1, ProcessCompletion::Spawned, Quiescence::Unknown);
    projection
        .apply(&state, captured_event(&scope, 4, spawned))
        .unwrap();
    let before = projection.clone();
    let mut wrong = checkpoint_capture(&scope, &spec, 2);
    wrong.process.as_mut().unwrap().start_ticks += 1;
    assert!(
        projection
            .apply(&state, captured_event(&scope, 5, wrong))
            .is_err()
    );
    let mut terminal = observation(
        &scope,
        3,
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
    assert!(
        projection
            .apply(&state, captured_event(&scope, 5, terminal.clone()))
            .is_err()
    );
    assert!(projection == before);
    projection
        .apply(
            &state,
            captured_event(&scope, 5, checkpoint_capture(&scope, &spec, 2)),
        )
        .unwrap();
    assert!(projection.effects()[0].pending());
    assert_eq!(projection.effects()[0].report, None);
    let before = projection.clone();
    assert!(
        projection
            .apply(
                &state,
                captured_event(&scope, 6, checkpoint_capture(&scope, &spec, 3))
            )
            .is_err()
    );
    assert!(projection == before);
    projection
        .apply(&state, captured_event(&scope, 6, terminal))
        .unwrap();
    assert_eq!(
        projection.effects()[0].report,
        Some(EffectOutcome::Succeeded)
    );
    assert!(projection.effects()[0].pending());
}
#[test]
fn cancellation_expiry_and_unknown_are_checkpoint_barriers() {
    for case in 0..3 {
        let (state, mut projection, scope, spec) = checkpoint_setup();
        projection
            .apply(
                &state,
                captured_event(
                    &scope,
                    4,
                    observation(&scope, 1, ProcessCompletion::Spawned, Quiescence::Unknown),
                ),
            )
            .unwrap();
        let at = match case {
            0 => {
                projection
                    .apply(
                        &state,
                        EffectEvent::Cancel {
                            effect: intent(&scope).id,
                            scope: scope.clone(),
                            at: 5,
                            basis: source("cancel-before-checkpoint"),
                        },
                    )
                    .unwrap();
                6
            }
            1 => scope_grant(&scope).expires_before,
            _ => {
                projection
                    .apply(
                        &state,
                        captured_event(
                            &scope,
                            5,
                            observation(&scope, 2, ProcessCompletion::Unknown, Quiescence::Unknown),
                        ),
                    )
                    .unwrap();
                6
            }
        };
        let before = projection.clone();
        assert!(
            projection
                .apply(
                    &state,
                    captured_event(&scope, at, checkpoint_capture(&scope, &spec, 3))
                )
                .is_err()
        );
        assert!(projection == before);
    }
}

#[test]
fn declared_pool_cannot_supply_missing_boot_correspondence_for_a_scope_sample() {
    use cortex_core::program::{domain::PoolIdentity, memory::*};
    let mut captures = Vec::new();
    for boot in ["boot-a", "boot-b"] {
        let resource = Resource::memory(
            MemoryBasis::CgroupCharged,
            MemoryScope::AccountingPool(PoolIdentity {
                physical: "pool".into(),
                incarnation: "allocation".into(),
                host: "host".into(),
                boot: boot.into(),
            }),
        )
        .unwrap();
        let (state, _, scope) = setup();
        let mut projection = OperationalProjection::new_supervised_fixture();
        let mut selected_policy = scope_policy();
        for capacity in &mut selected_policy.capacities {
            if matches!(capacity.resource, Resource::MemoryBytes(_)) {
                capacity.resource = resource.clone();
            }
        }
        let mut grant = scope_grant(&scope);
        let mut selected_intent = scope_intent(&scope);
        for demand in grant.ceilings.iter_mut().chain(&mut selected_intent.demand) {
            if matches!(demand.resource, Resource::MemoryBytes(_)) {
                demand.resource = resource.clone();
            }
        }
        let mut spec = contained_spec(&scope);
        spec.containment.as_mut().unwrap().memory_resource = match &resource {
            Resource::MemoryBytes(memory) => Some(memory.clone()),
            _ => unreachable!(),
        };
        for event in [
            EffectEvent::Policy(selected_policy),
            EffectEvent::Grant(grant),
            EffectEvent::Reserve {
                intent: selected_intent,
                at: 1,
            },
            EffectEvent::PrepareProcess {
                effect: intent(&scope).id,
                scope: scope.clone(),
                at: 2,
                spec: Box::new(spec.clone()),
            },
        ] {
            projection.apply(&state, event).unwrap();
        }
        // A declared reservation is retained before dispatch, not observed usage.
        assert_eq!(projection.accounted(resource.clone()).unwrap(), 536_870_912);
        dispatch(&state, &mut projection, &scope);
        let mut terminal = observation(
            &scope,
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
        let captured = terminal.containment.as_ref().unwrap().charged_memory_peak();
        assert_eq!(captured.bytes(), Some(1_048_576));
        assert_eq!(captured.basis(), MemoryBasis::CgroupCharged);
        assert!(captured.resource().is_none());
        captures.push(terminal.containment.clone());
        observed(&state, &mut projection, &scope, terminal.clone(), 4);
        assert_eq!(
            projection.accounted(resource.clone()),
            Err(OperationalError::UnknownResource(resource.clone()))
        );
        let recovery = projection
            .inspect_process_recovery(&intent(&scope).id, &scope)
            .unwrap();
        assert!(recovery.reservation_held);
        assert_eq!(recovery.blockers, vec![RecoveryBlocker::MissingUsage]);
        assert!(recovery.candidate.is_none());
        let mut proposed_usage = usage(&terminal);
        proposed_usage.push(Demand {
            resource: Resource::CpuMillicores,
            amount: Some(1000),
        });
        proposed_usage.push(Demand {
            resource,
            amount: captured.bytes(),
        });
        let before = projection.clone();
        assert!(
            projection
                .apply(
                    &state,
                    EffectEvent::Reconcile {
                        effect: intent(&scope).id,
                        scope: scope.clone(),
                        at: 5,
                        resolution: Reconciliation::Observed {
                            outcome: EffectOutcome::Succeeded,
                            usage: proposed_usage
                        },
                        basis: source("insufficient-scope-attribution")
                    }
                )
                .is_err()
        );
        assert!(projection == before);
        assert!(projection.effects()[0].pending());
    }
    // The legacy sample is byte-for-byte equivalent under two incompatible
    // declarations. Neither declaration proves the sample's host/boot scope.
    assert!(captures[0] == captures[1]);
}
