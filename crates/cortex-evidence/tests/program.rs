//! GPE-B D1/V1: actual A-1 consumption, candidate-scoped refusal and replay.
#[path = "support/program.rs"]
mod fixture;
use cortex_core::{binding::*, parallel::*, program::*};
use cortex_evidence::{program::*, unified::*};
use fixture::*;

#[test]
fn conflicting_grants_within_one_handoff_cannot_enter_the_journal() {
    let mut kernel = CoordinationKernel::new(definition()).unwrap();
    admit(
        &mut kernel,
        "claim",
        Command::Claim(attempt("old", "A", "operator")),
    );
    let before = kernel.operations().to_vec();
    let mut sender = grant(1, "operator");
    sender.id = GrantId::new("fresh-shared-id").unwrap();
    let mut recipient = grant(1, "recipient");
    recipient.id = sender.id.clone();
    let input = Operation {
        id: OperationId::new("handoff").unwrap(),
        expected: kernel.revision(),
        grant: sender,
        command: Command::Handoff {
            old: AttemptId::new("old").unwrap(),
            recipient,
            next: attempt("new", "A", "recipient"),
        },
    };
    assert!(kernel.admit(input).is_err());
    assert!(kernel.operations() == before);
    assert_eq!(kernel.state().attempts()[0].status, AttemptStatus::Claimed);
}

#[test]
fn restoring_original_scope_representation_cannot_change_semantic_identity() {
    let kernel = initialized();
    let scope = cortex_evidence::readiness::EdgeScopeSnapshot::capture(
        kernel.state().readiness(),
        &EdgeId::new("AB").unwrap(),
    )
    .unwrap();
    assert!(
        scope
            .clone()
            .with_original_scopes(scope.producer().clone(), scope.consumer().clone())
            .is_ok()
    );
    assert!(
        scope
            .clone()
            .with_original_scopes(scope.consumer().clone(), scope.producer().clone())
            .is_err()
    );
}

#[test]
fn intermediate_release_fan_out_fan_in_and_no_mission_completion() {
    let mut k = initialized();
    admit(
        &mut k,
        "claim-A",
        Command::Claim(attempt("A-attempt", "A", "operator")),
    );
    release(&mut k, "AB");
    assert!(
        k.state()
            .readiness()
            .mission(&task("B"))
            .unwrap()
            .eligible()
    );
    assert_eq!(k.state().attempts()[0].status, AttemptStatus::Claimed);
    assert!(
        !k.state()
            .readiness()
            .mission(&task("F"))
            .unwrap()
            .eligible()
    );
    release(&mut k, "BF");
    assert!(
        !k.state()
            .readiness()
            .mission(&task("F"))
            .unwrap()
            .dependency_ready()
    );
    assert!(!matches!(
        k.state()
            .readiness()
            .edge(&EdgeId::new("BC").unwrap())
            .unwrap()
            .decision(),
        EdgeDecision::Released { .. }
    ));
    release(&mut k, "DE");
    release(&mut k, "EF");
    assert!(
        k.state()
            .readiness()
            .mission(&task("F"))
            .unwrap()
            .eligible()
    );
    assert_eq!(k.state().authority(), ExecutionAuthority::Unavailable);
    assert_eq!(
        k.state().readiness().mission(&task("A")).unwrap().status(),
        MissionStatus::Active
    );
}
#[test]
fn unassessed_and_missing_context_do_not_release_then_new_valid_decision_can() {
    let mut k = initialized();
    let r = prepare_release(&mut k, "AB", false);
    admit(&mut k, "held", Command::Release(Box::new(r.clone())));
    assert!(!k.decisions()[0].released);
    assert!(
        !k.state()
            .readiness()
            .mission(&task("B"))
            .unwrap()
            .eligible()
    );
    let assessment = AssessmentInput {
        examination: r.examinations[0].clone(),
        id: AssessmentId::new("assess-AB").unwrap(),
        at: std::time::UNIX_EPOCH + std::time::Duration::from_secs(3),
        result: AssessmentResult::Pass,
        applicability: Applicability::Applicable,
        rationale: "Complete scoped observation".into(),
    };
    admit(
        &mut k,
        "assess-later",
        Command::Assess(Box::new(assessment)),
    );
    let mut missing = r.clone();
    missing.consumer = None;
    admit(&mut k, "held-context", Command::Release(Box::new(missing)));
    assert!(!k.decisions()[1].released);
    admit(&mut k, "fresh-decision", Command::Release(Box::new(r)));
    assert!(k.decisions()[2].released);
}
#[test]
fn raw_review_cannot_bypass_evidence_and_replay_does_not_repeat_release() {
    let mut k = initialized();
    let r = prepare_release(&mut k, "AB", true);
    let n = k.revision();
    assert!(
        k.admit(operation(&k, "bypass", Command::Notify(r.review.clone())))
            .is_err()
    );
    assert_eq!(k.revision(), n);
    let op = operation(&k, "release", Command::Release(Box::new(r)));
    k.admit(op.clone()).unwrap();
    assert!(k.admit(op.clone()).unwrap().duplicate);
    assert_eq!(k.decisions().len(), 1);
    let mut conflict = op;
    conflict.command = Command::Claim(attempt("bad", "A", "operator"));
    assert!(k.admit(conflict).is_err());
    assert_eq!(
        k.state()
            .readiness()
            .edge(&EdgeId::new("AB").unwrap())
            .unwrap()
            .releases()
            .len(),
        1
    );
}
#[test]
fn immutable_grants_and_expected_versions_reject_atomic_conflicts() {
    let mut k = initialized();
    let mut stale = operation(
        &k,
        "stale",
        Command::Claim(attempt("stale", "A", "operator")),
    );
    stale.expected = 0;
    let n = k.revision();
    assert!(k.admit(stale).is_err());
    let mut forged = operation(
        &k,
        "forged",
        Command::Claim(attempt("forged", "A", "operator")),
    );
    forged.grant.permissions = vec![CoordinationPermission::Claim];
    assert!(k.admit(forged).is_err());
    assert_eq!(k.revision(), n);
    assert!(k.state().attempts().is_empty());
}
#[test]
fn amendment_keeps_explicit_nonimpact_release_and_invalidates_only_closure() {
    let mut k = initialized();
    release(&mut k, "AB");
    release(&mut k, "DE");
    let op = operation(
        &k,
        "amend",
        Command::Amend {
            next: Box::new(definition_revision(2, true)),
            non_impact: NonImpact {
                missions: vec![task("D"), task("E")],
                basis: source("complete-independent-branch-coverage"),
            },
        },
    );
    k.admit(op).unwrap();
    assert!(
        !k.state()
            .readiness()
            .mission(&task("B"))
            .unwrap()
            .eligible()
    );
    assert!(
        k.state()
            .readiness()
            .mission(&task("E"))
            .unwrap()
            .eligible()
    );
    assert_eq!(k.decisions().len(), 2);
    assert_eq!(k.state().definitions().len(), 2);
}
#[test]
fn false_nonimpact_refuses_without_partially_amending() {
    let mut k = initialized();
    let n = k.revision();
    let op = operation(
        &k,
        "amend",
        Command::Amend {
            next: Box::new(definition_revision(2, true)),
            non_impact: NonImpact {
                missions: vec![task("B"), task("D"), task("E")],
                basis: source("unproved-neighbor"),
            },
        },
    );
    assert!(k.admit(op).is_err());
    assert_eq!(k.revision(), n);
    assert_eq!(k.state().definitions().len(), 1);
}
#[test]
fn explicit_stale_examination_holds_consumers_but_retains_terminal_pass() {
    let mut k = initialized();
    let r = prepare_release(&mut k, "AB", true);
    admit(&mut k, "release", Command::Release(Box::new(r.clone())));
    let mut ex = r.examinations[0].clone();
    let old = ex.contract.clone();
    ex.contract =
        CandidateBindingContract::new(old.task().clone(), rev(2), old.candidate().clone());
    ex.candidate = Some(capture(&ex.contract, 3));
    let before = k.revision();
    let inspection = k.inspect_record(&ex).unwrap();
    assert_eq!(inspection.validity, Some(Validity::Stale));
    assert_eq!(k.revision(), before);
    assert!(
        k.state()
            .readiness()
            .mission(&task("B"))
            .unwrap()
            .eligible()
    );
    admit(&mut k, "retain-stale", Command::Examine(ex));
    assert!(
        !k.state()
            .readiness()
            .mission(&task("B"))
            .unwrap()
            .eligible()
    );
    assert_eq!(
        k.records()[0].assessment().unwrap().result(),
        AssessmentResult::Pass
    );
    assert!(
        k.state()
            .readiness()
            .edge(&EdgeId::new("AB").unwrap())
            .unwrap()
            .releases()[0]
            .invalidated_by()
            .is_some()
    );
}
#[test]
fn unchanged_examination_is_a_legitimate_noninvalidating_neighbor() {
    let mut k = initialized();
    let r = prepare_release(&mut k, "AB", true);
    admit(&mut k, "release", Command::Release(Box::new(r.clone())));
    admit(
        &mut k,
        "unchanged",
        Command::Examine(r.examinations[0].clone()),
    );
    assert!(
        k.state()
            .readiness()
            .mission(&task("B"))
            .unwrap()
            .eligible()
    );
    assert_eq!(
        k.state()
            .readiness()
            .scope(&task("B"))
            .unwrap()
            .generation(),
        rev(1)
    );
}
#[test]
fn replay_rebuilds_exact_inputs_decisions_and_unknown_attempts() {
    let mut k = initialized();
    release(&mut k, "AB");
    admit(
        &mut k,
        "claim",
        Command::Claim(attempt("one", "A", "operator")),
    );
    admit(
        &mut k,
        "unknown",
        Command::Attempt {
            id: AttemptId::new("one").unwrap(),
            generation: rev(1),
            status: AttemptStatus::Unknown,
            basis: source("interruption"),
        },
    );
    let mut replay = CoordinationKernel::new(k.initial().clone()).unwrap();
    for o in k.operations() {
        replay.admit(o.clone()).unwrap();
    }
    assert!(replay.operations() == k.operations());
    assert!(replay.decisions() == k.decisions());
    assert!(replay.records() == k.records());
    assert_eq!(replay.state().attempts()[0].status, AttemptStatus::Unknown);
}

#[test]
fn unsupported_new_proposal_does_not_revoke_a_previously_valid_release() {
    let mut k = initialized();
    let r = prepare_release(&mut k, "AB", true);
    admit(&mut k, "release", Command::Release(Box::new(r.clone())));
    let mut missing = r;
    missing.consumer = None;
    admit(&mut k, "held-proposal", Command::Release(Box::new(missing)));
    assert!(!k.decisions().last().unwrap().released);
    assert!(
        k.state()
            .readiness()
            .mission(&task("B"))
            .unwrap()
            .eligible()
    );
}
#[test]
fn old_evidence_does_not_revoke_a_newer_independently_supported_review() {
    let mut k = initialized();
    let first = prepare_release(&mut k, "AB", true);
    admit(
        &mut k,
        "release-first",
        Command::Release(Box::new(first.clone())),
    );
    let original = k.records()[0].observation().clone();
    let observation = ObservationId::new("evidence-AB", 2).unwrap();
    let report = ReportedObservation::new(
        observation.clone(),
        original.subject().clone(),
        original.profile().clone(),
        original.times(),
        original.kind().clone(),
        original.detail(),
    )
    .unwrap();
    admit(&mut k, "observe-second", Command::Observe(report));
    let mut ex = first.examinations[0].clone();
    ex.observation = observation.clone();
    let assessment = AssessmentId::new("assessment-second").unwrap();
    admit(
        &mut k,
        "assess-second",
        Command::Assess(Box::new(AssessmentInput {
            examination: ex.clone(),
            id: assessment.clone(),
            at: std::time::UNIX_EPOCH + std::time::Duration::from_secs(4),
            result: AssessmentResult::Pass,
            applicability: Applicability::Applicable,
            rationale: "Independent retained observation".into(),
        })),
    );
    let mut second = first.clone();
    second.examinations = vec![ex];
    second.bindings = first
        .bindings
        .iter()
        .map(|b| {
            cortex_evidence::readiness::WitnessBinding::new(
                b.predicate().clone(),
                cortex_evidence::readiness::EvidenceReference::new(
                    observation.clone(),
                    Some(assessment.clone()),
                ),
                b.artifact().clone(),
            )
        })
        .collect();
    second.review = Notification::new(
        NotificationId::new("review-second").unwrap(),
        first.review.graph().clone(),
        first.review.event().clone(),
    )
    .unwrap();
    admit(&mut k, "release-second", Command::Release(Box::new(second)));
    admit(
        &mut k,
        "invalidate-old",
        Command::InvalidateEvidence {
            observation: original.id().clone(),
            reason: "Old mechanism corrected".into(),
        },
    );
    assert!(
        k.state()
            .readiness()
            .mission(&task("B"))
            .unwrap()
            .eligible()
    );
}
