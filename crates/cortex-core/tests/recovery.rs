//! Pure recovery explanations; no host process or current-identity assertion.
#[path = "support/program.rs"]
mod fixture;
#[path = "support/supervision.rs"]
mod support;
use cortex_core::program::{operational::*, supervision::*};
use support::*;

#[test]
fn dispatch_without_receipt_is_unknown_and_inspection_does_not_change_it() {
    let (state, mut p, s) = setup();
    prepare(&state, &mut p, &s);
    dispatch(&state, &mut p, &s);
    let before = p.clone();
    let v = p.inspect_process_recovery(&intent(&s).id, &s).unwrap();
    assert!(v.reservation_held);
    assert!(v.candidate.is_none());
    assert_eq!(v.blockers, [RecoveryBlocker::MissingReceipt]);
    assert!(p == before);
}
#[test]
fn genuine_pre_spawn_refusal_is_a_candidate_not_an_automatic_refund() {
    let (state, mut p, s) = setup();
    prepare(&state, &mut p, &s);
    dispatch(&state, &mut p, &s);
    observed(
        &state,
        &mut p,
        &s,
        observation(
            &s,
            1,
            ProcessCompletion::StartFailed,
            Quiescence::NoChildStarted,
        ),
        4,
    );
    let v = p.inspect_process_recovery(&intent(&s).id, &s).unwrap();
    assert!(v.candidate == Some(Reconciliation::Absent));
    assert!(v.reservation_held);
    assert!(p.has_pending_attempt(&s.attempt));
    assert_eq!(
        p.accounted(cortex_core::program::scheduling::Resource::WorkerSlots)
            .unwrap(),
        1
    );
}
#[test]
fn parent_exit_with_retained_handles_cannot_become_a_proof_candidate() {
    let (state, mut p, s) = setup();
    prepare(&state, &mut p, &s);
    dispatch(&state, &mut p, &s);
    observed(
        &state,
        &mut p,
        &s,
        observation(
            &s,
            1,
            ProcessCompletion::Exited {
                code: Some(0),
                signal: None,
            },
            Quiescence::Unknown,
        ),
        4,
    );
    let v = p.inspect_process_recovery(&intent(&s).id, &s).unwrap();
    assert!(v.candidate.is_none());
    assert!(v.blockers.contains(&RecoveryBlocker::UnprovenQuiescence));
}
#[test]
fn valid_direct_child_capture_uses_existing_guard_and_keeps_original_until_admission() {
    let (state, mut p, s) = setup();
    prepare(&state, &mut p, &s);
    dispatch(&state, &mut p, &s);
    observed(
        &state,
        &mut p,
        &s,
        observation(
            &s,
            1,
            ProcessCompletion::Exited {
                code: Some(0),
                signal: None,
            },
            Quiescence::DirectChildReapedStreamsClosed,
        ),
        4,
    );
    let before = p.clone();
    let view = p.inspect_process_recovery(&intent(&s).id, &s).unwrap();
    assert!(view.blockers.is_empty());
    assert!(view.reservation_held);
    assert!(p == before);
    p.apply(
        &state,
        EffectEvent::Reconcile {
            effect: intent(&s).id,
            scope: s.clone(),
            at: 5,
            resolution: view.candidate.unwrap(),
            basis: source("separately-authorized-fixture-reconciliation"),
        },
    )
    .unwrap();
    assert!(!p.effects()[0].pending());
    let settled = p.inspect_process_recovery(&intent(&s).id, &s).unwrap();
    assert!(!settled.reservation_held && settled.candidate.is_none());
    assert!(p.effects()[0].process_observations == before.effects()[0].process_observations);
}
#[test]
fn scope_without_identity_and_unresolved_helper_stays_held_after_replay() {
    let (state, _, s) = setup();
    let mut p = OperationalProjection::new_supervised_fixture();
    for e in [
        EffectEvent::Policy(scope_policy()),
        EffectEvent::Grant(scope_grant(&s)),
        EffectEvent::Reserve {
            intent: scope_intent(&s),
            at: 1,
        },
        EffectEvent::PrepareProcess {
            effect: intent(&s).id,
            scope: s.clone(),
            at: 2,
            spec: Box::new(contained_spec(&s)),
        },
    ] {
        p.apply(&state, e).unwrap();
    }
    dispatch(&state, &mut p, &s);
    let mut o = observation(
        &s,
        1,
        ProcessCompletion::Interrupted {
            reason: InterruptionReason::InputClosed,
        },
        Quiescence::Unknown,
    );
    o.input_bytes = 0;
    o.stdout_bytes = 0;
    o.structured_records = 0;
    o.unresolved_scope_helpers = vec![ScopeHelperIdentity {
        pid: 99,
        start_ticks: Some(42),
    }];
    observed(&state, &mut p, &s, o, 4);
    let mut replay = OperationalProjection::new_supervised_fixture();
    for e in p.events() {
        replay.apply(&state, e.clone()).unwrap();
    }
    let v = replay.inspect_process_recovery(&intent(&s).id, &s).unwrap();
    assert!(v.reservation_held && v.launcher_recorded && !v.scope_recorded);
    assert!(!v.identity_checkpoint_recorded && v.candidate.is_none());
    assert!(v.blockers.contains(&RecoveryBlocker::MissingScopeIdentity));
    assert!(v.blockers.contains(&RecoveryBlocker::UnresolvedHelper));
    assert_eq!(replay.effects()[0].status, EffectStatus::Unknown);
    assert!(replay == p);
}
#[test]
fn foreign_scope_cannot_request_inspection_of_another_effect() {
    let (_, p, mut s) = setup();
    s.generation = rev(2);
    assert!(p.inspect_process_recovery(&intent(&s).id, &s).is_err());
}
#[test]
fn no_dispatch_and_pure_fixture_are_not_dispatched_recovery_candidates() {
    let (_, p, s) = setup();
    let v = p.inspect_process_recovery(&intent(&s).id, &s).unwrap();
    assert_eq!(v.blockers, [RecoveryBlocker::NoDispatch]);
    assert!(v.candidate.is_none());
    assert!(
        OperationalProjection::new()
            .inspect_process_recovery(&intent(&s).id, &s)
            .is_err()
    );
}
