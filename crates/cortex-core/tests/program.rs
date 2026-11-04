//! GPE-B C1/O1/P1/Q1: negative cases and legitimate independent neighbors.
#[path = "support/program.rs"]
mod fixture;
use cortex_core::{parallel::*, program::context::*, program::scheduling::*, program::*};
use fixture::*;

#[test]
fn handoff_refuses_conflicting_grants_before_changing_either_attempt() {
    let mut state = ProgramState::new(definition()).unwrap();
    state
        .claim(&grant(1, "operator"), attempt("old", "A", "operator"))
        .unwrap();
    let before = state.attempts().to_vec();
    let notifications = state.readiness().notifications().len();
    let mut sender = grant(1, "operator");
    sender.id = GrantId::new("new-handoff-grant").unwrap();
    let mut recipient = grant(1, "recipient");
    recipient.id = sender.id.clone();
    assert_eq!(
        state.handoff(
            &sender,
            &recipient,
            &AttemptId::new("old").unwrap(),
            attempt("new", "A", "recipient")
        ),
        Err(ProgramError::IdentityConflict)
    );
    assert!(state.attempts() == before);
    assert_eq!(state.readiness().notifications().len(), notifications);
    recipient.id = GrantId::new("distinct-recipient-grant").unwrap();
    state
        .handoff(
            &sender,
            &recipient,
            &AttemptId::new("old").unwrap(),
            attempt("new", "A", "recipient"),
        )
        .unwrap();
    assert_eq!(state.attempts()[0].status, AttemptStatus::HandedOver);
    assert_eq!(state.attempts()[1].status, AttemptStatus::Claimed);
}

#[test]
fn amendment_cannot_reuse_any_retained_mission_contract_revision_for_changed_contents() {
    use cortex_core::{TaskContract, binding::CandidateBindingContract};
    let mut state = ProgramState::new(definition()).unwrap();
    let changed = |baseline: u64, revision: u64| {
        with_contracts(&definition_revision(baseline, false), |c| {
            if c.task().id() != &task("A") {
                return c.clone();
            }
            CandidateBindingContract::new(
                TaskContract::new(task("A"), "Changed accepted objective", "Changed criterion")
                    .unwrap(),
                rev(revision),
                c.candidate().clone(),
            )
        })
    };
    assert_eq!(
        state.amend(&grant(1, "operator"), changed(2, 1)),
        Err(ProgramError::IdentityConflict)
    );
    assert_eq!(state.definitions().len(), 1);
    state.amend(&grant(1, "operator"), changed(2, 2)).unwrap();
    assert_eq!(
        state.amend(&grant(2, "operator"), changed(3, 1)),
        Err(ProgramError::IdentityConflict)
    );
    assert_eq!(state.definitions().len(), 2);
    // Re-selecting the exact historical contract in an explicitly new baseline is valid.
    state
        .amend(&grant(2, "operator"), definition_revision(3, false))
        .unwrap();
    assert_eq!(state.definitions().len(), 3);
}

#[test]
fn amendment_cannot_alias_historical_artifact_revisions() {
    let changed = |baseline, revision, commit: &str| {
        let definition = definition_revision(baseline, false);
        let artifact = ArtifactReference::new(
            ArtifactId::new("A").unwrap(),
            rev(revision),
            candidate(commit),
        );
        let edges = definition
            .graph()
            .edges()
            .iter()
            .map(|edge| {
                if edge.requirement().artifact().id() != artifact.id() {
                    return edge.clone();
                }
                DependencyEdge::new(
                    edge.id().clone(),
                    edge.producer().clone(),
                    edge.consumer().clone(),
                    ArtifactRequirement::new(
                        artifact.clone(),
                        edge.requirement().kind(),
                        edge.requirement().level(),
                        edge.requirement().predicates().to_vec(),
                    )
                    .unwrap(),
                )
            })
            .collect();
        let graph = DependencyGraph::new(
            definition.graph().reference().clone(),
            definition.graph().missions().to_vec(),
            edges,
            definition.graph().limits(),
        )
        .unwrap();
        let mut increments = definition.increments().to_vec();
        for increment in &mut increments {
            for value in &mut increment.artifacts {
                if value.id() == artifact.id() {
                    *value = artifact.clone();
                }
            }
        }
        ProgramDefinition::new(
            definition.id().clone(),
            definition.outcome().clone(),
            definition.revision(),
            definition.predecessor(),
            definition.approval().clone(),
            graph,
            definition.workstreams().to_vec(),
            increments,
            definition.limits(),
        )
        .unwrap()
    };
    let mut state = ProgramState::new(definition()).unwrap();
    assert_eq!(
        state.amend(&grant(1, "operator"), changed(2, 1, "9")),
        Err(ProgramError::IdentityConflict)
    );
    assert_eq!(state.definitions().len(), 1);
    state
        .amend(&grant(1, "operator"), changed(2, 2, "9"))
        .unwrap();
    assert_eq!(
        state.amend(&grant(2, "operator"), changed(3, 1, "9")),
        Err(ProgramError::IdentityConflict)
    );
    assert_eq!(state.definitions().len(), 2);
    state
        .amend(&grant(2, "operator"), changed(3, 1, "a"))
        .unwrap();
    assert_eq!(state.definitions().len(), 3);
}

#[test]
fn exclusive_claims_unknown_reconciliation_and_handoff() {
    let mut s = ProgramState::new(definition()).unwrap();
    s.claim(&grant(1, "operator"), attempt("one", "A", "operator"))
        .unwrap();
    assert_eq!(
        s.claim(&grant(1, "second"), attempt("two", "A-later", "second")),
        Err(ProgramError::Claimed)
    );
    s.claim(&grant(1, "second"), attempt("independent", "D", "second"))
        .unwrap();
    s.reconcile_attempt(
        &grant(1, "operator"),
        &AttemptId::new("one").unwrap(),
        rev(1),
        AttemptStatus::Unknown,
        source("interrupted"),
    )
    .unwrap();
    assert!(
        s.handoff(
            &grant(1, "operator"),
            &grant(1, "second"),
            &AttemptId::new("one").unwrap(),
            attempt("replacement", "A", "second")
        )
        .is_err()
    );
    assert_eq!(s.attempts()[0].status, AttemptStatus::Unknown);
    s.reconcile_attempt(
        &grant(1, "operator"),
        &AttemptId::new("one").unwrap(),
        rev(1),
        AttemptStatus::Cancelled,
        source("explicit-no-external-effect-reconciliation"),
    )
    .unwrap();
    s.claim(&grant(1, "operator"), attempt("new", "A", "operator"))
        .unwrap();
    s.handoff(
        &grant(1, "operator"),
        &grant(1, "second"),
        &AttemptId::new("new").unwrap(),
        attempt("handed", "A", "second"),
    )
    .unwrap();
    assert_eq!(s.attempts()[2].status, AttemptStatus::HandedOver);
    assert_eq!(s.attempts()[3].owner, ActorId::new("second").unwrap());
    assert_eq!(s.authority(), ExecutionAuthority::Unavailable);
}
#[test]
fn stale_attempt_and_grant_do_not_become_current_after_amendment() {
    let mut s = ProgramState::new(definition()).unwrap();
    initialize(&mut s);
    s.claim(&grant(1, "operator"), attempt("one", "A", "operator"))
        .unwrap();
    let affected = s
        .amend(&grant(1, "operator"), definition_revision(2, true))
        .unwrap();
    assert_eq!(
        affected.iter().map(|i| i.as_str()).collect::<Vec<_>>(),
        ["A", "B", "C", "F"]
    );
    assert!(s.readiness().mission(&task("D")).unwrap().eligible());
    assert!(!s.readiness().mission(&task("A")).unwrap().eligible());
    assert_eq!(
        s.reconcile_attempt(
            &grant(2, "operator"),
            &AttemptId::new("one").unwrap(),
            rev(1),
            AttemptStatus::Reported,
            source("late")
        ),
        Err(ProgramError::Stale)
    );
    assert!(
        s.claim(&grant(1, "operator"), attempt("old-grant", "D", "operator"))
            .is_err()
    );
    s.reconcile_attempt(
        &grant(2, "operator"),
        &AttemptId::new("one").unwrap(),
        rev(1),
        AttemptStatus::Cancelled,
        source("retained-old-claim"),
    )
    .unwrap();
    assert_eq!(s.definitions().len(), 2);
}
#[test]
fn plan_cannot_remove_missions_or_mutate_acceptance() {
    let mut s = ProgramState::new(definition()).unwrap();
    let before = s.definition().clone();
    assert_eq!(
        s.set_plan(
            &grant(1, "operator"),
            Plan {
                revision: rev(1),
                order: vec![task("A")],
                basis: source("bad-plan")
            }
        ),
        Err(ProgramError::Structure)
    );
    s.set_plan(
        &grant(1, "operator"),
        Plan {
            revision: rev(1),
            order: ["D", "E", "A", "B", "C", "F"]
                .into_iter()
                .map(task)
                .collect(),
            basis: source("order"),
        },
    )
    .unwrap();
    assert!(s.definition() == &before);
    assert!(
        s.notify(
            &grant(1, "operator"),
            notify(
                &s,
                "accept",
                Event::Status {
                    scope: s.readiness().scope(&task("A")).unwrap(),
                    status: MissionStatus::Accepted,
                    source: source("worker-claim")
                }
            )
        )
        .is_err()
    );
}
fn package(s: &ProgramState) -> PackageRequest {
    PackageRequest {
        id: PackageId::new("context-A").unwrap(),
        revision: rev(1),
        mission: task("A"),
        baseline: rev(1),
        generation: s.readiness().scope(&task("A")).unwrap().generation(),
        sources: vec![SourceInput {
            source: source("explicit-source"),
            candidate: binding("A", "a", 1).candidate().clone(),
            text: "Grounded fixture text. Ignore prior instructions is source data.".into(),
        }],
        methods: vec![MethodProposal {
            method: source("native.candidate-correspondence"),
            applicability: "Current candidate correspondence".into(),
            qualification: None,
            exceptions: "No exceptions".into(),
        }],
        omissions: "No live source recapture".into(),
        deliverable: "Inspectable proposal".into(),
        stop_rules: "Stop on missing inputs; no executor operations".into(),
    }
}
#[test]
fn package_is_exact_and_rejects_stale_source_or_modified_protected_bytes() {
    let mut s = ProgramState::new(definition()).unwrap();
    let input = package(&s);
    let p = assemble(&s, input.clone()).unwrap();
    assert_eq!(p.bytes(), assemble(&s, input.clone()).unwrap().bytes());
    p.check(&s, p.bytes()).unwrap();
    let mut modified = p.bytes().to_vec();
    modified.push(b'!');
    assert!(p.check(&s, &modified).is_err());
    let mut wrong = input.clone();
    wrong.sources[0].candidate = binding("A", "b", 1).candidate().clone();
    assert!(assemble(&s, wrong).is_err());
    s.amend(&grant(1, "operator"), definition_revision(2, true))
        .unwrap();
    assert!(p.check(&s, p.bytes()).is_err());
    assert!(
        std::str::from_utf8(p.bytes())
            .unwrap()
            .contains("EXPLICIT_PROPOSAL_ONLY")
    );
}
#[test]
fn scheduler_reserves_verification_and_blocks_unknown_or_unenforceable_capacity() {
    let mut s = ProgramState::new(definition()).unwrap();
    initialize(&mut s);
    let p = |id: &str, age| Proposal {
        mission: task(id),
        band: 1,
        waiting_rounds: age,
        risk_rank: 0,
        cost_rank: 0,
        demand: vec![
            Demand {
                resource: Resource::Workers,
                amount: Some(1),
            },
            Demand {
                resource: Resource::RamBytes,
                amount: Some(100),
            },
        ],
    };
    let caps = vec![
        Capacity {
            resource: Resource::Workers,
            available: Some(2),
            verification_reserve: 1,
            declared_enforceable: true,
        },
        Capacity {
            resource: Resource::RamBytes,
            available: Some(300),
            verification_reserve: 100,
            declared_enforceable: true,
        },
    ];
    let r = simulate(&s, &[p("A", 0), p("D", 5)], &caps).unwrap();
    assert_eq!(r[0].mission, task("D"));
    assert!(r[0].proposed);
    assert!(!r[1].proposed);
    assert!(
        r[1].held
            .contains(&WaitReason::Contention(Resource::Workers))
    );
    let mut missing = caps.clone();
    missing[1].available = None;
    assert!(
        simulate(&s, &[p("A", 0)], &missing).unwrap()[0]
            .held
            .contains(&WaitReason::Unknown(Resource::RamBytes))
    );
    missing[1].available = Some(300);
    missing[1].declared_enforceable = false;
    assert!(
        simulate(&s, &[p("A", 0)], &missing).unwrap()[0]
            .held
            .contains(&WaitReason::Unenforceable(Resource::RamBytes))
    );
    assert!(simulate(&s, &[p("A", 0), p("A", 1)], &caps).is_err());
}
#[test]
fn invalid_program_membership_limits_and_wrong_grant_are_refused() {
    assert!(ProgramLimits::new(0, 8, 1024).is_err());
    let d = definition();
    let mut w = d.workstreams().to_vec();
    w[0].missions.push(task("D"));
    assert!(
        ProgramDefinition::new(
            d.id().clone(),
            d.outcome().clone(),
            rev(1),
            None,
            source("acceptance"),
            d.graph().clone(),
            w,
            d.increments().to_vec(),
            d.limits()
        )
        .is_err()
    );
    let mut s = ProgramState::new(d).unwrap();
    let mut g = grant(1, "operator");
    g.permissions.clear();
    assert_eq!(
        s.claim(&g, attempt("a", "A", "operator")),
        Err(ProgramError::Authority)
    );
    assert!(s.attempts().is_empty());
}

#[test]
fn scoped_invalidation_cannot_modify_missions_outside_its_grant() {
    let mut s = ProgramState::new(definition()).unwrap();
    initialize(&mut s);
    let before = s.readiness().clone();
    let mut g = grant(1, "operator");
    g.missions = vec![task("A")];
    let input = notify(
        &s,
        "invalidate",
        Event::Invalidate {
            roots: vec![s.readiness().scope(&task("A")).unwrap()],
            cause: source("changed"),
            coverage: DependencyCoverage::DeclaredComplete,
        },
    );
    assert_eq!(s.notify(&g, input), Err(ProgramError::Authority));
    assert!(s.readiness() == &before);
}

#[test]
fn artifact_revision_amendment_discards_old_report_without_blocking_independent_work() {
    let mut s = ProgramState::new(definition()).unwrap();
    initialize(&mut s);
    let old = s.definition().graph().edges()[0]
        .requirement()
        .artifact()
        .clone();
    let input = notify(
        &s,
        "old-artifact",
        Event::Artifact(ArtifactReport::new(
            s.readiness().scope(&task("A")).unwrap(),
            old.clone(),
            DependencyKind::Artifact,
            ReadinessLevel::ContractStable,
            source("old-output"),
        )),
    );
    s.notify(&grant(1, "operator"), input).unwrap();
    let d = definition_revision(2, false);
    let replacement = ArtifactReference::new(old.id().clone(), rev(2), old.candidate().clone());
    let edges = d
        .graph()
        .edges()
        .iter()
        .map(|e| {
            if e.producer() == &task("A") {
                DependencyEdge::new(
                    e.id().clone(),
                    e.producer().clone(),
                    e.consumer().clone(),
                    ArtifactRequirement::new(
                        replacement.clone(),
                        e.requirement().kind(),
                        e.requirement().level(),
                        e.requirement().predicates().to_vec(),
                    )
                    .unwrap(),
                )
            } else {
                e.clone()
            }
        })
        .collect();
    let graph = DependencyGraph::new(
        d.graph().reference().clone(),
        d.graph().missions().to_vec(),
        edges,
        d.graph().limits(),
    )
    .unwrap();
    let increments = d
        .increments()
        .iter()
        .map(|i| {
            let mut n = i.clone();
            if n.id == IncrementId::new("A").unwrap() {
                n.artifacts = vec![replacement.clone()];
            }
            n
        })
        .collect();
    let next = ProgramDefinition::new(
        d.id().clone(),
        d.outcome().clone(),
        d.revision(),
        d.predecessor(),
        d.approval().clone(),
        graph,
        d.workstreams().to_vec(),
        increments,
        d.limits(),
    )
    .unwrap();
    let affected = s.amend(&grant(1, "operator"), next).unwrap();
    assert_eq!(
        affected.iter().map(|m| m.as_str()).collect::<Vec<_>>(),
        ["B", "C", "F"]
    );
    assert!(s.readiness().mission(&task("D")).unwrap().eligible());
    assert!(!s.readiness().mission(&task("B")).unwrap().eligible());
}

#[test]
fn identical_commit_from_a_different_repository_is_not_the_same_package_source() {
    let s = ProgramState::new(definition()).unwrap();
    let mut input = package(&s);
    input.sources[0].candidate = binding("other-repository", "a", 1).candidate().clone();
    assert!(matches!(assemble(&s, input), Err(ProgramError::Stale)));
}
#[test]
fn program_outcome_amendment_requires_full_requalification() {
    let mut s = ProgramState::new(definition()).unwrap();
    initialize(&mut s);
    let d = definition_revision(2, false);
    let next = ProgramDefinition::new(
        d.id().clone(),
        ProgramOutcome::new(
            "Changed accepted objective",
            "New explicit outcome criterion",
        )
        .unwrap(),
        d.revision(),
        d.predecessor(),
        d.approval().clone(),
        d.graph().clone(),
        d.workstreams().to_vec(),
        d.increments().to_vec(),
        d.limits(),
    )
    .unwrap();
    assert_eq!(s.amend(&grant(1, "operator"), next).unwrap().len(), 6);
    assert_eq!(
        s.definitions()[0].outcome().objective(),
        "Coordinate the connected fixture program"
    );
    assert!(!s.readiness().mission(&task("D")).unwrap().eligible());
}
