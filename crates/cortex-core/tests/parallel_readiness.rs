//! G1 public-consumer contracts: literal DAG/transition oracles, no I/O or authority.

use cortex_core::{
    CandidateId, TaskContract, TaskId, binding::*, parallel::*, temporal::SourceReference,
};

fn task(s: &str) -> TaskId {
    TaskId::new(s).unwrap()
}
fn eid(s: &str) -> EdgeId {
    EdgeId::new(s).unwrap()
}
fn nid(s: &str) -> NotificationId {
    NotificationId::new(s).unwrap()
}
fn rev(n: u64) -> ContractRevision {
    ContractRevision::new(n).unwrap()
}
fn candidate(hex: &str) -> CandidateId {
    CandidateId::from_git_commit(&hex.repeat(40)).unwrap()
}
fn source(s: &str) -> SourceReference {
    SourceReference::operator_reported("synthetic-operator", s, "1").unwrap()
}
fn graph_ref(n: u64) -> GraphReference {
    GraphReference::new(GraphId::new("program").unwrap(), rev(n))
}
fn limits() -> GraphLimits {
    GraphLimits::new(6, 5, 256, 4096, 2_000_000).unwrap()
}

fn context(id: &str, hex: &str, revision: u64) -> MissionContext {
    // Construct synthetic absolute paths only; no directory or Git object is created.
    let root = std::env::temp_dir().join(format!("g1-synthetic-{id}"));
    let repo = RepositoryIdentity::new(root.clone(), root.join(".git"), root.join(".git")).unwrap();
    let c = CandidateBindingContract::new(
        TaskContract::new(
            task(id),
            "Produce the contracted outcome",
            "Retain exact output and support",
        )
        .unwrap(),
        rev(revision),
        CandidateRequirement::new(repo.clone(), candidate(hex), WorktreeRequirement::Clean),
    );
    let state = RepositoryState::new(
        Some(repo),
        Head::Detached {
            commit: candidate(hex),
        },
        vec![],
        ObservationCoverage::GitStatus,
        ObservationConsistency::TwoMatchingSamples,
    )
    .unwrap();
    let capture = CandidateObservation::new(
        ObservationId::new(&format!("capture-{id}"), 1).unwrap(),
        &c,
        state,
        Some(ComparisonBasis::new("synthetic-profile", b"asserted comparison".to_vec()).unwrap()),
    );
    MissionContext::new(c, Some(capture)).unwrap()
}
fn missions() -> Vec<Mission> {
    [
        ("A", "a"),
        ("B", "b"),
        ("C", "c"),
        ("D", "d"),
        ("E", "e"),
        ("F", "f"),
    ]
    .into_iter()
    .map(|(id, hex)| {
        Mission::new(
            context(id, hex, 1),
            ConsumerKind::Preview,
            DependencyCoverage::DeclaredComplete,
        )
    })
    .collect()
}
fn artifact(id: &str, revision: u64, hex: &str) -> ArtifactReference {
    ArtifactReference::new(ArtifactId::new(id).unwrap(), rev(revision), candidate(hex))
}
fn dependency(
    id: &str,
    endpoints: (&str, &str),
    output: ArtifactReference,
    kind: DependencyKind,
    level: ReadinessLevel,
    predicates: &[&str],
) -> DependencyEdge {
    DependencyEdge::new(
        eid(id),
        task(endpoints.0),
        task(endpoints.1),
        ArtifactRequirement::new(
            output,
            kind,
            level,
            predicates
                .iter()
                .map(|p| PredicateId::new(p).unwrap())
                .collect(),
        )
        .unwrap(),
    )
}
fn edges() -> Vec<DependencyEdge> {
    use DependencyKind::Artifact;
    use ReadinessLevel::IntegratedAccepted;
    vec![
        dependency(
            "AB",
            ("A", "B"),
            artifact("a1", 1, "1"),
            Artifact,
            IntegratedAccepted,
            &["pAB"],
        ),
        dependency(
            "BC",
            ("B", "C"),
            artifact("b1", 1, "2"),
            Artifact,
            IntegratedAccepted,
            &["pBC"],
        ),
        dependency(
            "BF",
            ("B", "F"),
            artifact("b1", 1, "2"),
            Artifact,
            IntegratedAccepted,
            &["pBF"],
        ),
        dependency(
            "DE",
            ("D", "E"),
            artifact("d1", 1, "3"),
            Artifact,
            IntegratedAccepted,
            &["pDE"],
        ),
        dependency(
            "EF",
            ("E", "F"),
            artifact("e1", 1, "4"),
            Artifact,
            IntegratedAccepted,
            &["pEF"],
        ),
    ]
}
fn projection() -> ReadinessProjection {
    ReadinessProjection::new(
        DependencyGraph::new(graph_ref(1), missions(), edges(), limits()).unwrap(),
    )
    .unwrap()
}
fn notification(p: &ReadinessProjection, id: &str, event: Event) -> Notification {
    Notification::new(nid(id), p.graph().reference().clone(), event).unwrap()
}
fn apply(p: &mut ReadinessProjection, id: &str, event: Event) -> TransitionReport {
    p.apply(notification(p, id, event)).unwrap()
}
fn ready(p: &mut ReadinessProjection, id: &str, name: &str) {
    apply(
        p,
        id,
        Event::Preconditions {
            scope: p.scope(&task(name)).unwrap(),
            ready: true,
            source: source(id),
        },
    );
}
fn slots(p: &mut ReadinessProjection, id: &str, available: usize) {
    apply(
        p,
        id,
        Event::Capacity {
            available,
            source: source(id),
        },
    );
}
fn start_roots(p: &mut ReadinessProjection) {
    for name in ["A", "B", "C", "D", "E", "F"] {
        ready(p, &format!("ready-{name}"), name);
    }
    slots(p, "initial-slots", 2);
}
fn status(p: &mut ReadinessProjection, id: &str, name: &str, status: MissionStatus) {
    apply(
        p,
        id,
        Event::Status {
            scope: p.scope(&task(name)).unwrap(),
            status,
            source: source(id),
        },
    );
}
fn report(
    p: &mut ReadinessProjection,
    id: &str,
    name: &str,
    output: ArtifactReference,
    kind: DependencyKind,
    level: ReadinessLevel,
) -> NotificationId {
    let value = ArtifactReport::new(
        p.scope(&task(name)).unwrap(),
        output,
        kind,
        level,
        source(id),
    );
    apply(p, id, Event::Artifact(value));
    nid(id)
}
fn attest(provisional: bool) -> ReviewAttestation {
    ReviewAttestation::new(
        source("reviewer"),
        source("approval"),
        provisional.then(|| {
            ProvisionalUse::new(source("risk"), source("revalidate"), source("cost-owner"))
        }),
    )
}
fn support(id: &str, tag: &str) -> PredicateSupport {
    PredicateSupport::new(
        PredicateId::new(id).unwrap(),
        source(&format!("{tag}-observation")),
        source(&format!("{tag}-assessment")),
        source(&format!("{tag}-context")),
    )
}
fn review(
    p: &ReadinessProjection,
    edge: &str,
    report: NotificationId,
    predicates: &[&str],
    role: ReviewRole,
    provisional: bool,
) -> Event {
    let pair = match edge {
        "AB" => ("A", "B"),
        "BC" => ("B", "C"),
        "BF" => ("B", "F"),
        "DE" => ("D", "E"),
        "EF" => ("E", "F"),
        _ => panic!("unknown synthetic edge"),
    };
    Event::Review(Box::new(EdgeReview::new(
        eid(edge),
        report,
        p.scope(&task(pair.0)).unwrap(),
        p.scope(&task(pair.1)).unwrap(),
        role,
        attest(provisional),
        predicates.iter().map(|id| support(id, edge)).collect(),
    )))
}
fn release(
    p: &mut ReadinessProjection,
    id: &str,
    edge: &str,
    report: NotificationId,
    predicates: &[&str],
) -> TransitionReport {
    let event = review(p, edge, report, predicates, ReviewRole::Coordinator, false);
    apply(p, id, event)
}
fn eligible(p: &ReadinessProjection, name: &str) -> bool {
    p.mission(&task(name)).unwrap().eligible()
}
fn released(p: &ReadinessProjection, name: &str) -> bool {
    matches!(
        p.edge(&eid(name)).unwrap().decision(),
        EdgeDecision::Released { .. }
    )
}
fn expect_atomic(p: &mut ReadinessProjection, n: Notification, error: ReadinessError) {
    let before = p.clone();
    assert_eq!(p.apply(n), Err(error));
    assert_eq!(*p, before);
}
fn prime() -> ReadinessProjection {
    let mut p = projection();
    start_roots(&mut p);
    status(&mut p, "active-A", "A", MissionStatus::Active);
    let a = report(
        &mut p,
        "report-A",
        "A",
        artifact("a1", 1, "1"),
        DependencyKind::Artifact,
        ReadinessLevel::IntegratedAccepted,
    );
    release(&mut p, "review-AB", "AB", a, &["pAB"]);
    status(&mut p, "active-B", "B", MissionStatus::Active);
    let b = report(
        &mut p,
        "report-B",
        "B",
        artifact("b1", 1, "2"),
        DependencyKind::Artifact,
        ReadinessLevel::IntegratedAccepted,
    );
    release(&mut p, "review-BC", "BC", b.clone(), &["pBC"]);
    release(&mut p, "review-BF", "BF", b, &["pBF"]);
    let d = report(
        &mut p,
        "report-D",
        "D",
        artifact("d1", 1, "3"),
        DependencyKind::Artifact,
        ReadinessLevel::IntegratedAccepted,
    );
    release(&mut p, "review-DE", "DE", d, &["pDE"]);
    let e = report(
        &mut p,
        "report-E",
        "E",
        artifact("e1", 1, "4"),
        DependencyKind::Artifact,
        ReadinessLevel::IntegratedAccepted,
    );
    release(&mut p, "review-EF", "EF", e, &["pEF"]);
    p
}

#[test]
fn g1_01_to_04_roots_intermediate_chain_independent_branch_and_required_fan_in() {
    let mut p = projection();
    for name in ["A", "B", "C", "D", "E", "F"] {
        assert!(!eligible(&p, name));
    }
    assert!(p.mission(&task("A")).unwrap().dependency_ready());
    assert!(!p.mission(&task("B")).unwrap().dependency_ready());
    start_roots(&mut p);
    assert!(eligible(&p, "A") && eligible(&p, "D"));
    for name in ["B", "C", "E", "F"] {
        assert!(!eligible(&p, name));
    }
    status(&mut p, "A-started", "A", MissionStatus::Active);
    apply(
        &mut p,
        "signal",
        Event::Signal {
            mission: task("A"),
            message: "a1 is ready; please begin".into(),
        },
    );
    assert!(!released(&p, "AB") && !eligible(&p, "B"));
    let a = report(
        &mut p,
        "a-report",
        "A",
        artifact("a1", 1, "1"),
        DependencyKind::Artifact,
        ReadinessLevel::IntegratedAccepted,
    );
    assert!(!eligible(&p, "B"));
    let transition = release(&mut p, "a-reviewed", "AB", a.clone(), &["pAB"]);
    assert_eq!(transition.changed_edges(), &[eid("AB")]);
    assert_eq!(transition.changed_missions(), &[task("B")]);
    assert_eq!(transition.authority(), ExecutionAuthority::Unavailable);
    assert!(eligible(&p, "B"));
    assert_eq!(
        p.mission(&task("A")).unwrap().status(),
        MissionStatus::Active
    );
    assert!(!eligible(&p, "A"));
    assert_eq!(p.edge(&eid("AB")).unwrap().releases()[0].report(), &a);
    assert_ne!(
        artifact("a1", 1, "1").candidate(),
        p.scope(&task("A"))
            .unwrap()
            .context()
            .contract()
            .candidate()
            .commit()
    );

    status(&mut p, "B-started", "B", MissionStatus::Active);
    let b = report(
        &mut p,
        "b-report",
        "B",
        artifact("b1", 1, "2"),
        DependencyKind::Artifact,
        ReadinessLevel::IntegratedAccepted,
    );
    release(&mut p, "b-for-c", "BC", b.clone(), &["pBC"]);
    assert!(eligible(&p, "C"));
    assert!(!released(&p, "BF") && !eligible(&p, "F"));
    release(&mut p, "b-for-f", "BF", b, &["pBF"]);
    assert!(!eligible(&p, "F"));
    let d = report(
        &mut p,
        "d-report",
        "D",
        artifact("d1", 1, "3"),
        DependencyKind::Artifact,
        ReadinessLevel::IntegratedAccepted,
    );
    release(&mut p, "d-for-e", "DE", d, &["pDE"]);
    assert!(eligible(&p, "E"));
    assert!(!eligible(&p, "F"));
    let e = report(
        &mut p,
        "e-report",
        "E",
        artifact("e1", 1, "4"),
        DependencyKind::Artifact,
        ReadinessLevel::IntegratedAccepted,
    );
    release(&mut p, "e-for-f", "EF", e, &["pEF"]);
    assert!(eligible(&p, "F"));
    assert_eq!(
        p.mission(&task("B")).unwrap().status(),
        MissionStatus::Active
    );
    for edge in ["AB", "BC", "BF", "DE", "EF"] {
        assert!(released(&p, edge));
        assert_eq!(
            p.edge(&eid(edge)).unwrap().authority(),
            ExecutionAuthority::Unavailable
        );
    }
}

#[test]
fn g1_05_sibling_edges_require_their_own_predicates_and_revisions() {
    let mut es = edges();
    es.retain(|e| e.id() != &eid("BF"));
    es.push(dependency(
        "BF",
        ("B", "F"),
        artifact("b1", 2, "5"),
        DependencyKind::Artifact,
        ReadinessLevel::IntegratedAccepted,
        &["pBF"],
    ));
    let mut p = ReadinessProjection::new(
        DependencyGraph::new(graph_ref(1), missions(), es, limits()).unwrap(),
    )
    .unwrap();
    let b = report(
        &mut p,
        "b-v1",
        "B",
        artifact("b1", 1, "2"),
        DependencyKind::Artifact,
        ReadinessLevel::IntegratedAccepted,
    );
    release(&mut p, "for-c", "BC", b.clone(), &["pBC"]);
    assert!(released(&p, "BC"));
    let n = notification(
        &p,
        "bad-revision",
        review(&p, "BF", b, &["pBF"], ReviewRole::Coordinator, false),
    );
    expect_atomic(
        &mut p,
        n,
        ReadinessError::ReviewRejected(ReviewRefusal::ArtifactContract),
    );
    let b2 = report(
        &mut p,
        "b-v2",
        "B",
        artifact("b1", 2, "5"),
        DependencyKind::Artifact,
        ReadinessLevel::IntegratedAccepted,
    );
    release(&mut p, "missing-predicate", "BF", b2.clone(), &[]);
    assert_eq!(
        p.edge(&eid("BF")).unwrap().decision(),
        &EdgeDecision::Held(HoldReason::MissingSupport)
    );
    let n = notification(
        &p,
        "wrong-predicate",
        review(
            &p,
            "BF",
            b2.clone(),
            &["pBC"],
            ReviewRole::Coordinator,
            false,
        ),
    );
    expect_atomic(
        &mut p,
        n,
        ReadinessError::ReviewRejected(ReviewRefusal::Predicate),
    );
    release(&mut p, "right-predicate", "BF", b2, &["pBF"]);
    assert!(released(&p, "BC") && released(&p, "BF"));
}

#[test]
fn g1_06_levels_are_exact_and_provisional_preview_requires_explicit_policy() {
    for level in [
        ReadinessLevel::ContractStable,
        ReadinessLevel::ImplementationProvisional,
    ] {
        let mut ms = missions();
        ms[1] = Mission::new(
            context("B", "b", 1),
            ConsumerKind::Effectful,
            DependencyCoverage::DeclaredComplete,
        );
        let es = vec![dependency(
            "AB",
            ("A", "B"),
            artifact("a1", 1, "1"),
            DependencyKind::Artifact,
            level,
            &["pAB"],
        )];
        assert_eq!(
            DependencyGraph::new(graph_ref(1), ms, es, limits()).unwrap_err(),
            ReadinessError::Unsupported(UnsupportedOperation::Dispatch)
        );
    }
    let mut p = projection();
    let a = report(
        &mut p,
        "provisional-a",
        "A",
        artifact("a1", 1, "1"),
        DependencyKind::Artifact,
        ReadinessLevel::ImplementationProvisional,
    );
    let n = notification(
        &p,
        "no-downgrade",
        review(&p, "AB", a, &["pAB"], ReviewRole::Coordinator, true),
    );
    expect_atomic(
        &mut p,
        n,
        ReadinessError::ReviewRejected(ReviewRefusal::ReadinessLevel),
    );

    for level in [
        ReadinessLevel::ContractStable,
        ReadinessLevel::ImplementationProvisional,
    ] {
        let es = vec![dependency(
            "AB",
            ("A", "B"),
            artifact("a1", 1, "1"),
            DependencyKind::Artifact,
            level,
            &["pAB"],
        )];
        let mut p = ReadinessProjection::new(
            DependencyGraph::new(graph_ref(1), missions(), es, limits()).unwrap(),
        )
        .unwrap();
        start_roots(&mut p);
        let a = report(
            &mut p,
            "a",
            "A",
            artifact("a1", 1, "1"),
            DependencyKind::Artifact,
            level,
        );
        let event = review(
            &p,
            "AB",
            a.clone(),
            &["pAB"],
            ReviewRole::Coordinator,
            false,
        );
        apply(&mut p, "first", event);
        if level == ReadinessLevel::ImplementationProvisional {
            assert_eq!(
                p.edge(&eid("AB")).unwrap().decision(),
                &EdgeDecision::Held(HoldReason::ProvisionalPolicy)
            );
            let event = review(&p, "AB", a, &["pAB"], ReviewRole::Coordinator, true);
            apply(&mut p, "with-policy", event);
        }
        assert!(eligible(&p, "B"));
        assert_eq!(
            p.mission(&task("B")).unwrap().authority(),
            ExecutionAuthority::Unavailable
        );
    }
}

#[test]
fn g1_07_and_10_invalidation_has_causal_closure_and_requires_fresh_context_reviews() {
    let mut p = prime();
    assert!(eligible(&p, "C") && eligible(&p, "E") && eligible(&p, "F"));
    let old = p.notification(&nid("review-AB")).unwrap().clone();
    let d = p.mission(&task("D")).unwrap().clone();
    let e = p.mission(&task("E")).unwrap().clone();
    let event = Event::Invalidate {
        roots: vec![p.scope(&task("A")).unwrap()],
        cause: source("oracle-change-v2"),
        coverage: DependencyCoverage::DeclaredComplete,
    };
    let transition = apply(&mut p, "invalidate-A", event);
    assert_eq!(
        transition.changed_missions(),
        &[task("A"), task("B"), task("C"), task("F")]
    );
    assert_eq!(
        transition.changed_edges(),
        &[eid("AB"), eid("BC"), eid("BF"), eid("EF")]
    );
    assert_eq!(p.mission(&task("D")).unwrap(), &d);
    assert_eq!(p.mission(&task("E")).unwrap(), &e);
    assert!(released(&p, "DE"));
    for name in ["B", "C", "F"] {
        assert!(!p.mission(&task(name)).unwrap().dependency_ready());
    }
    assert_eq!(p.scope(&task("A")).unwrap().generation(), rev(2));
    let before = p.clone();
    assert!(p.apply(old.clone()).unwrap().duplicate());
    assert_eq!(p, before);
    assert!(!released(&p, "AB"));
    let stale =
        Notification::new(nid("old-context-new-id"), graph_ref(1), old.event().clone()).unwrap();
    expect_atomic(&mut p, stale, ReadinessError::StaleContext);
    let stale_report = notification(
        &p,
        "old-report-new-review",
        review(
            &p,
            "AB",
            nid("report-A"),
            &["pAB"],
            ReviewRole::Coordinator,
            false,
        ),
    );
    expect_atomic(&mut p, stale_report, ReadinessError::StaleContext);

    let a = report(
        &mut p,
        "new-A-report",
        "A",
        artifact("a1", 1, "1"),
        DependencyKind::Artifact,
        ReadinessLevel::IntegratedAccepted,
    );
    release(&mut p, "new-AB-review", "AB", a, &["pAB"]);
    let records = p.edge(&eid("AB")).unwrap().releases();
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].invalidated_by(), Some(&nid("invalidate-A")));
    assert_eq!(records[1].invalidated_by(), None);
    assert!(p.mission(&task("B")).unwrap().dependency_ready());
    assert!(!eligible(&p, "B")); // still Active
    let b = report(
        &mut p,
        "new-B-report",
        "B",
        artifact("b1", 1, "2"),
        DependencyKind::Artifact,
        ReadinessLevel::IntegratedAccepted,
    );
    release(&mut p, "new-BC", "BC", b.clone(), &["pBC"]);
    assert!(!eligible(&p, "C")); // preconditions also stale
    ready(&mut p, "new-C-preconditions", "C");
    assert!(eligible(&p, "C"));
    release(&mut p, "new-BF", "BF", b, &["pBF"]);
    // E's report remains on its unaffected context, but F needs a new per-edge review.
    release(&mut p, "new-EF", "EF", nid("report-E"), &["pEF"]);
    ready(&mut p, "new-F-preconditions", "F");
    assert!(eligible(&p, "F"));
    assert_eq!(p.mission(&task("D")).unwrap(), &d);
    assert_eq!(p.mission(&task("E")).unwrap(), &e);
}

#[test]
fn g1_08_capacity_changes_never_erase_release_basis_or_grant_authority() {
    let mut p = prime();
    let edge = p.edge(&eid("BC")).unwrap().clone();
    assert!(eligible(&p, "C"));
    slots(&mut p, "no-slots", 0);
    assert!(p.mission(&task("C")).unwrap().dependency_ready());
    assert!(!eligible(&p, "C"));
    assert!(
        p.mission(&task("C"))
            .unwrap()
            .blockers()
            .contains(&MissionBlocker::NoCapacity)
    );
    assert_eq!(p.edge(&eid("BC")).unwrap(), &edge);
    slots(&mut p, "slots-restored", 1);
    assert!(eligible(&p, "C"));
    assert_eq!(p.authority(), ExecutionAuthority::Unavailable);
    let n = notification(
        &p,
        "no-launch",
        Event::Unsupported(UnsupportedOperation::Dispatch),
    );
    expect_atomic(
        &mut p,
        n,
        ReadinessError::Unsupported(UnsupportedOperation::Dispatch),
    );
}

#[test]
fn g1_09_duplicate_conflict_and_rejected_identity_reuse_are_atomic() {
    let mut p = projection();
    let n = notification(
        &p,
        "signal",
        Event::Signal {
            mission: task("A"),
            message: "information".into(),
        },
    );
    let first = p.apply(n.clone()).unwrap();
    let before = p.clone();
    let again = p.apply(n).unwrap();
    assert!(again.duplicate());
    assert_eq!(again.sequence(), first.sequence());
    assert_eq!(p, before);
    let conflict = notification(
        &p,
        "signal",
        Event::Signal {
            mission: task("A"),
            message: "different".into(),
        },
    );
    expect_atomic(&mut p, conflict, ReadinessError::IdentityConflict);
    let absent = notification(
        &p,
        "reusable",
        Event::Signal {
            mission: task("absent"),
            message: "information".into(),
        },
    );
    expect_atomic(&mut p, absent, ReadinessError::MissingReference);
    apply(
        &mut p,
        "reusable",
        Event::Signal {
            mission: task("A"),
            message: "corrected".into(),
        },
    );
    assert_eq!(p.notifications().len(), 2);
    let a = report(
        &mut p,
        "a",
        "A",
        artifact("a1", 1, "1"),
        DependencyKind::Artifact,
        ReadinessLevel::IntegratedAccepted,
    );
    let bad = notification(
        &p,
        "review",
        review(&p, "AB", a.clone(), &["pAB"], ReviewRole::Producer, false),
    );
    expect_atomic(
        &mut p,
        bad,
        ReadinessError::ReviewRejected(ReviewRefusal::NotCoordinator),
    );
    release(&mut p, "review", "AB", a, &["pAB"]);
    assert!(released(&p, "AB"));
}

#[test]
fn g1_09_graph_refuses_cycles_self_edges_missing_and_ambiguous_identities() {
    let ms = missions();
    let es = edges();
    let mut duplicate = ms.clone();
    duplicate.push(ms[0].clone());
    assert_eq!(
        DependencyGraph::new(
            graph_ref(1),
            duplicate,
            es.clone(),
            GraphLimits::new(7, 5, 10, 100, 1_000_000).unwrap()
        )
        .unwrap_err(),
        ReadinessError::InvalidGraph(GraphIssue::DuplicateMission)
    );
    let mut duplicate = es.clone();
    duplicate[1] = es[0].clone();
    assert_eq!(
        DependencyGraph::new(graph_ref(1), ms.clone(), duplicate, limits()).unwrap_err(),
        ReadinessError::InvalidGraph(GraphIssue::DuplicateEdge)
    );
    for (pair, issue) in [
        (("A", "A"), GraphIssue::SelfEdge),
        (("A", "absent"), GraphIssue::MissingEndpoint),
    ] {
        let d = dependency(
            "bad",
            pair,
            artifact("bad-output", 1, "1"),
            DependencyKind::Artifact,
            ReadinessLevel::IntegratedAccepted,
            &["p"],
        );
        assert_eq!(
            DependencyGraph::new(graph_ref(1), ms.clone(), vec![d], limits()).unwrap_err(),
            ReadinessError::InvalidGraph(issue)
        );
    }
    let cycle = vec![
        dependency(
            "AB",
            ("A", "B"),
            artifact("a", 1, "1"),
            DependencyKind::Artifact,
            ReadinessLevel::IntegratedAccepted,
            &["p"],
        ),
        dependency(
            "BA",
            ("B", "A"),
            artifact("b", 1, "2"),
            DependencyKind::Artifact,
            ReadinessLevel::IntegratedAccepted,
            &["q"],
        ),
    ];
    assert_eq!(
        DependencyGraph::new(graph_ref(1), ms.clone(), cycle, limits()).unwrap_err(),
        ReadinessError::InvalidGraph(GraphIssue::Cycle)
    );
    let alias = vec![
        dependency(
            "AB",
            ("A", "B"),
            artifact("shared", 1, "1"),
            DependencyKind::Artifact,
            ReadinessLevel::IntegratedAccepted,
            &["p"],
        ),
        dependency(
            "DC",
            ("D", "C"),
            artifact("shared", 2, "2"),
            DependencyKind::Artifact,
            ReadinessLevel::IntegratedAccepted,
            &["q"],
        ),
    ];
    assert_eq!(
        DependencyGraph::new(graph_ref(1), ms, alias, limits()).unwrap_err(),
        ReadinessError::InvalidGraph(GraphIssue::ArtifactAlias)
    );
    assert!(
        ArtifactRequirement::new(
            artifact("x", 1, "1"),
            DependencyKind::Artifact,
            ReadinessLevel::IntegratedAccepted,
            vec![PredicateId::new("p").unwrap(); 2]
        )
        .is_err()
    );
}

#[test]
fn g1_10_unknown_coverage_never_becomes_ready_through_new_flags_or_reviews() {
    let mut p = prime();
    let roots = vec![p.scope(&task("A")).unwrap()];
    apply(
        &mut p,
        "unknown-impact",
        Event::Invalidate {
            roots,
            cause: source("unestablished-impact"),
            coverage: DependencyCoverage::Unknown,
        },
    );
    for name in ["A", "B", "C", "D", "E", "F"] {
        assert!(!p.mission(&task(name)).unwrap().dependency_ready());
        assert_eq!(p.scope(&task(name)).unwrap().generation(), rev(2));
        assert!(
            p.mission(&task(name))
                .unwrap()
                .blockers()
                .contains(&MissionBlocker::UnknownCoverage)
        );
    }
    assert!(!released(&p, "DE"));
    let d = report(
        &mut p,
        "new-D",
        "D",
        artifact("d1", 1, "3"),
        DependencyKind::Artifact,
        ReadinessLevel::IntegratedAccepted,
    );
    release(&mut p, "new-DE", "DE", d, &["pDE"]);
    ready(&mut p, "conditions-E", "E");
    assert_eq!(
        p.edge(&eid("DE")).unwrap().decision(),
        &EdgeDecision::Held(HoldReason::UnknownCoverage)
    );
    assert!(!eligible(&p, "E"));
}

#[test]
fn missing_capture_and_unknown_declared_dependencies_are_held_not_approved() {
    for missing_capture in [true, false] {
        let mut ms = missions();
        let original = context("A", "a", 1);
        let ctx = if missing_capture {
            MissionContext::new(original.contract().clone(), None).unwrap()
        } else {
            original
        };
        ms[0] = Mission::new(
            ctx,
            ConsumerKind::Preview,
            if missing_capture {
                DependencyCoverage::DeclaredComplete
            } else {
                DependencyCoverage::Unknown
            },
        );
        let mut p = ReadinessProjection::new(
            DependencyGraph::new(graph_ref(1), ms, edges(), limits()).unwrap(),
        )
        .unwrap();
        start_roots(&mut p);
        assert!(!eligible(&p, "A"));
        let a = report(
            &mut p,
            "a",
            "A",
            artifact("a1", 1, "1"),
            DependencyKind::Artifact,
            ReadinessLevel::IntegratedAccepted,
        );
        release(&mut p, "review", "AB", a, &["pAB"]);
        let expected = if missing_capture {
            HoldReason::MissingContext
        } else {
            HoldReason::UnknownCoverage
        };
        assert_eq!(
            p.edge(&eid("AB")).unwrap().decision(),
            &EdgeDecision::Held(expected)
        );
        assert!(!eligible(&p, "B"));
    }
}

#[test]
fn g1_11_cancellation_is_terminal_but_not_artifact_rollback_and_merge_is_not_acceptance() {
    let mut p = prime();
    let edge = p.edge(&eid("AB")).unwrap().clone();
    status(&mut p, "cancel-A", "A", MissionStatus::Cancelled);
    assert_eq!(p.edge(&eid("AB")).unwrap(), &edge);
    assert!(!eligible(&p, "A"));
    let n = notification(
        &p,
        "restart",
        Event::Status {
            scope: p.scope(&task("A")).unwrap(),
            status: MissionStatus::Pending,
            source: source("restart"),
        },
    );
    expect_atomic(&mut p, n, ReadinessError::InvalidTransition);
    let mut p = projection();
    apply(
        &mut p,
        "merge-message",
        Event::Signal {
            mission: task("A"),
            message: "clean Git merge; exit zero".into(),
        },
    );
    assert!(!released(&p, "AB"));
    for op in [
        UnsupportedOperation::ExternalEffect,
        UnsupportedOperation::ResponsibilityHandoff,
        UnsupportedOperation::GraphEdit,
    ] {
        let n = notification(&p, "unsupported", Event::Unsupported(op));
        expect_atomic(&mut p, n, ReadinessError::Unsupported(op));
    }
}

#[test]
fn whole_mission_acceptance_needs_its_distinct_report_status_and_new_edge_review() {
    let es = vec![dependency(
        "AB",
        ("A", "B"),
        artifact("accepted-A", 1, "1"),
        DependencyKind::MissionAcceptance,
        ReadinessLevel::IntegratedAccepted,
        &["all-exits"],
    )];
    let mut p = ReadinessProjection::new(
        DependencyGraph::new(graph_ref(1), missions(), es, limits()).unwrap(),
    )
    .unwrap();
    start_roots(&mut p);
    let n = notification(
        &p,
        "wrong-kind",
        Event::Artifact(ArtifactReport::new(
            p.scope(&task("A")).unwrap(),
            artifact("accepted-A", 1, "1"),
            DependencyKind::Artifact,
            ReadinessLevel::IntegratedAccepted,
            source("output"),
        )),
    );
    expect_atomic(&mut p, n, ReadinessError::IncompatibleReference);
    let a = report(
        &mut p,
        "acceptance-report",
        "A",
        artifact("accepted-A", 1, "1"),
        DependencyKind::MissionAcceptance,
        ReadinessLevel::IntegratedAccepted,
    );
    release(&mut p, "premature-review", "AB", a.clone(), &["all-exits"]);
    assert_eq!(
        p.edge(&eid("AB")).unwrap().decision(),
        &EdgeDecision::Held(HoldReason::ProducerNotAccepted)
    );
    status(&mut p, "active", "A", MissionStatus::Active);
    status(&mut p, "accepted", "A", MissionStatus::Accepted);
    assert!(!released(&p, "AB"));
    release(&mut p, "fresh-review", "AB", a, &["all-exits"]);
    assert!(released(&p, "AB") && eligible(&p, "B"));
    assert!(!eligible(&p, "A"));
}

#[test]
fn a_held_or_rejected_new_proposal_cannot_revoke_an_existing_valid_release() {
    let mut p = prime();
    let before = p.edge(&eid("BC")).unwrap().clone();
    let bad = notification(
        &p,
        "wrong-role",
        review(
            &p,
            "BC",
            nid("report-B"),
            &["pBC"],
            ReviewRole::Producer,
            false,
        ),
    );
    expect_atomic(
        &mut p,
        bad,
        ReadinessError::ReviewRejected(ReviewRefusal::NotCoordinator),
    );
    release(&mut p, "missing-support", "BC", nid("report-B"), &[]);
    assert_eq!(p.edge(&eid("BC")).unwrap(), &before);
    assert!(eligible(&p, "C"));
    assert!(p.notification(&nid("missing-support")).is_ok());
}

fn tiny_graph(bytes: usize, notifications: usize, supports: usize) -> DependencyGraph {
    // Literal graph byte oracle: g(1) + x/o/a(3) + SHA(40) + /r(2) + /r/g twice(8) = 54.
    let repo = RepositoryIdentity::new("/r".into(), "/r/g".into(), "/r/g".into()).unwrap();
    let c = CandidateBindingContract::new(
        TaskContract::new(task("x"), "o", "a").unwrap(),
        rev(1),
        CandidateRequirement::new(repo, candidate("a"), WorktreeRequirement::Clean),
    );
    DependencyGraph::new(
        GraphReference::new(GraphId::new("g").unwrap(), rev(1)),
        vec![Mission::new(
            MissionContext::new(c, None).unwrap(),
            ConsumerKind::Preview,
            DependencyCoverage::DeclaredComplete,
        )],
        vec![],
        GraphLimits::new(1, 1, notifications, supports, bytes).unwrap(),
    )
    .unwrap()
}

#[test]
fn exact_byte_support_and_notification_limits_refuse_without_eviction_or_identity_consumption() {
    let graph = tiny_graph(54, 2, 1);
    assert_eq!(graph.input_usage().bytes(), 54);
    assert_eq!(graph.input_usage().supports(), 0);
    let ms = graph.missions().to_vec();
    assert_eq!(
        DependencyGraph::new(
            graph.reference().clone(),
            ms,
            vec![],
            GraphLimits::new(1, 1, 2, 1, 53).unwrap()
        )
        .unwrap_err(),
        ReadinessError::Limit(LimitKind::Bytes)
    );
    // n(1) + graph g(1) + source a/s/v(3) = exactly five more retained bytes, one support.
    for (bytes, ok) in [(59, true), (58, false)] {
        let mut p = ReadinessProjection::new(tiny_graph(bytes, 2, 1)).unwrap();
        let n = Notification::new(
            nid("n"),
            p.graph().reference().clone(),
            Event::Capacity {
                available: 1,
                source: SourceReference::operator_reported("a", "s", "v").unwrap(),
            },
        )
        .unwrap();
        if ok {
            p.apply(n).unwrap();
            assert_eq!(p.input_usage().bytes(), 59);
            assert_eq!(p.input_usage().supports(), 1);
        } else {
            expect_atomic(&mut p, n, ReadinessError::Limit(LimitKind::Bytes));
        }
    }
    let mut p = ReadinessProjection::new(tiny_graph(1000, 2, 1)).unwrap();
    let n = Notification::new(
        nid("n"),
        p.graph().reference().clone(),
        Event::Capacity {
            available: 1,
            source: SourceReference::operator_reported("a", "s", "v").unwrap(),
        },
    )
    .unwrap();
    p.apply(n.clone()).unwrap();
    let excess =
        Notification::new(nid("m"), p.graph().reference().clone(), n.event().clone()).unwrap();
    expect_atomic(&mut p, excess, ReadinessError::Limit(LimitKind::Supports));
    let filler = Notification::new(
        nid("m"),
        p.graph().reference().clone(),
        Event::Signal {
            mission: task("x"),
            message: "z".into(),
        },
    )
    .unwrap();
    p.apply(filler).unwrap();
    assert_eq!(p.notifications().len(), 2);
    let before = p.clone();
    assert!(p.apply(n).unwrap().duplicate());
    assert_eq!(p, before);
    let full = Notification::new(
        nid("k"),
        p.graph().reference().clone(),
        Event::Signal {
            mission: task("x"),
            message: "z".into(),
        },
    )
    .unwrap();
    expect_atomic(
        &mut p,
        full,
        ReadinessError::Limit(LimitKind::Notifications),
    );
}

#[test]
fn graph_mission_edge_and_support_bounds_have_exact_positive_neighbors() {
    assert!(
        DependencyGraph::new(
            graph_ref(1),
            missions(),
            edges(),
            GraphLimits::new(6, 5, 1, 5, 2_000_000).unwrap()
        )
        .is_ok()
    );
    for (m, e, s, error) in [
        (5, 5, 5, LimitKind::Missions),
        (6, 4, 5, LimitKind::Edges),
        (6, 5, 4, LimitKind::Supports),
    ] {
        assert_eq!(
            DependencyGraph::new(
                graph_ref(1),
                missions(),
                edges(),
                GraphLimits::new(m, e, 1, s, 2_000_000).unwrap()
            )
            .unwrap_err(),
            ReadinessError::Limit(error)
        );
    }
    for args in [
        [0, 1, 1, 1, 1],
        [1, 0, 1, 1, 1],
        [1, 1, 0, 1, 1],
        [1, 1, 1, 0, 1],
        [1, 1, 1, 1, 0],
        [usize::MAX, 1, 1, 1, 1],
    ] {
        assert_eq!(
            GraphLimits::new(args[0], args[1], args[2], args[3], args[4]),
            Err(ReadinessError::InvalidLimits)
        );
    }
}

#[test]
fn review_support_budget_includes_all_three_predicate_references_and_attestations() {
    for (supports, ok) in [(7, true), (6, false)] {
        let ms = missions().into_iter().take(2).collect();
        let es = vec![edges().remove(0)];
        let mut p = ReadinessProjection::new(
            DependencyGraph::new(
                graph_ref(1),
                ms,
                es,
                GraphLimits::new(2, 1, 2, supports, 1_000_000).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        let a = report(
            &mut p,
            "a",
            "A",
            artifact("a1", 1, "1"),
            DependencyKind::Artifact,
            ReadinessLevel::IntegratedAccepted,
        );
        let n = notification(
            &p,
            "review",
            review(&p, "AB", a, &["pAB"], ReviewRole::Coordinator, false),
        );
        if ok {
            p.apply(n).unwrap();
            assert!(released(&p, "AB"));
            assert_eq!(p.input_usage().supports(), 7);
        } else {
            expect_atomic(&mut p, n, ReadinessError::Limit(LimitKind::Supports));
        }
    }
}

#[test]
fn malformed_keys_revisions_and_signal_text_are_rejected_with_valid_neighbors() {
    for value in ["", "-bad", "bad/path", "bad\0id", "é"] {
        assert!(ArtifactId::new(value).is_err());
        assert!(PredicateId::new(value).is_err());
    }
    assert!(PredicateId::new(&"a".repeat(128)).is_ok());
    assert!(PredicateId::new(&"a".repeat(129)).is_err());
    assert!(ContractRevision::new(0).is_err());
    let p = projection();
    for message in ["".into(), "bad\ttext".into(), "x".repeat(4097)] {
        assert!(
            Notification::new(
                nid("n"),
                graph_ref(1),
                Event::Signal {
                    mission: task("A"),
                    message
                }
            )
            .is_err()
        );
    }
    assert!(
        Notification::new(
            nid("n"),
            graph_ref(1),
            Event::Signal {
                mission: task("A"),
                message: "é".repeat(2048)
            }
        )
        .is_ok()
    );
    let n = Notification::new(
        nid("n"),
        graph_ref(2),
        Event::Signal {
            mission: task("A"),
            message: "wrong graph".into(),
        },
    )
    .unwrap();
    let mut p = p;
    expect_atomic(&mut p, n, ReadinessError::GraphMismatch);
}

#[test]
fn immutable_graph_replacement_cannot_import_old_context_or_notification_authority() {
    let old = prime();
    let old_report = old.notification(&nid("report-A")).unwrap().clone();
    let mut ms = missions();
    ms[0] = Mission::new(
        context("A", "5", 2),
        ConsumerKind::Preview,
        DependencyCoverage::DeclaredComplete,
    );
    let mut new = ReadinessProjection::new(
        DependencyGraph::new(graph_ref(2), ms, edges(), limits()).unwrap(),
    )
    .unwrap();
    expect_atomic(&mut new, old_report, ReadinessError::GraphMismatch);
    assert!(new.notifications().is_empty());
    assert!(!released(&new, "AB"));
    assert!(released(&old, "AB"));
    let n = notification(
        &new,
        "wrong-capture",
        Event::Preconditions {
            scope: old.scope(&task("A")).unwrap(),
            ready: true,
            source: source("old-context"),
        },
    );
    expect_atomic(&mut new, n, ReadinessError::IncompatibleReference);
    let old_context = context("A", "a", 1);
    let changed = CandidateBindingContract::new(
        TaskContract::new(task("A"), "changed objective", "changed criterion").unwrap(),
        rev(1),
        old_context.contract().candidate().clone(),
    );
    assert_eq!(
        MissionContext::new(changed, old_context.capture().cloned()).unwrap_err(),
        ReadinessError::ContractAlias
    );
}

#[test]
fn deterministic_graph_order_and_redacted_debug_preserve_explicit_private_getters() {
    let graph = DependencyGraph::new(graph_ref(1), missions(), edges(), limits()).unwrap();
    let mut ms = missions();
    ms.reverse();
    let mut es = edges();
    es.reverse();
    assert_eq!(
        DependencyGraph::new(graph_ref(1), ms, es, limits()).unwrap(),
        graph
    );
    let mut p = ReadinessProjection::new(graph).unwrap();
    let n = notification(
        &p,
        "private-notification",
        Event::Signal {
            mission: task("A"),
            message: "secret payload /private/location".into(),
        },
    );
    assert!(!format!("{n:?}").contains("secret"));
    assert!(!format!("{:?}", n.event()).contains("private"));
    p.apply(n).unwrap();
    let debug = format!(
        "{p:?} {:?} {:?} {:?}",
        p.graph(),
        p.scope(&task("A")).unwrap(),
        p.mission(&task("A")).unwrap()
    );
    for private in [
        "secret",
        "private/location",
        "Produce the contracted",
        "synthetic-profile",
        "capture-A",
    ] {
        assert!(!debug.contains(private));
    }
    let Event::Signal { message, .. } = p
        .notification(&nid("private-notification"))
        .unwrap()
        .event()
    else {
        panic!("signal retained")
    };
    assert_eq!(message, "secret payload /private/location");
    let error = GraphId::new("/private/location").unwrap_err();
    assert!(!format!("{error:?} {error}").contains("private/location"));
}

#[test]
fn missing_and_mismatched_review_references_refuse_without_consuming_identity() {
    let mut p = projection();
    for result in [
        p.edge(&eid("absent")).map(|_| ()),
        p.mission(&task("absent")).map(|_| ()),
        p.notification(&nid("absent")).map(|_| ()),
    ] {
        assert_eq!(result, Err(ReadinessError::MissingReference));
    }
    let n = notification(
        &p,
        "review",
        review(
            &p,
            "AB",
            nid("absent"),
            &["pAB"],
            ReviewRole::Coordinator,
            false,
        ),
    );
    expect_atomic(&mut p, n, ReadinessError::MissingReference);
    apply(
        &mut p,
        "information",
        Event::Signal {
            mission: task("A"),
            message: "not an artifact".into(),
        },
    );
    let n = notification(
        &p,
        "review",
        review(
            &p,
            "AB",
            nid("information"),
            &["pAB"],
            ReviewRole::Coordinator,
            false,
        ),
    );
    expect_atomic(&mut p, n, ReadinessError::IncompatibleReference);
    let a = report(
        &mut p,
        "a",
        "A",
        artifact("a1", 1, "1"),
        DependencyKind::Artifact,
        ReadinessLevel::IntegratedAccepted,
    );
    let n = notification(
        &p,
        "review",
        Event::Review(Box::new(EdgeReview::new(
            eid("AB"),
            a.clone(),
            p.scope(&task("A")).unwrap(),
            p.scope(&task("C")).unwrap(),
            ReviewRole::Coordinator,
            attest(false),
            vec![support("pAB", "AB")],
        ))),
    );
    expect_atomic(&mut p, n, ReadinessError::IncompatibleReference);
    let n = notification(
        &p,
        "review",
        review(
            &p,
            "AB",
            a.clone(),
            &["pAB", "pAB"],
            ReviewRole::Coordinator,
            false,
        ),
    );
    expect_atomic(
        &mut p,
        n,
        ReadinessError::ReviewRejected(ReviewRefusal::Predicate),
    );
    release(&mut p, "review", "AB", a, &["pAB"]);
    assert!(released(&p, "AB"));
}

#[test]
fn incomplete_captures_hold_and_known_candidate_contradictions_refuse() {
    let base = context("A", "a", 1);
    let old = base.capture().unwrap();
    let wrong = RepositoryState::new(
        old.state().repository().cloned(),
        Head::Detached {
            commit: candidate("b"),
        },
        vec![],
        ObservationCoverage::GitStatus,
        ObservationConsistency::TwoMatchingSamples,
    )
    .unwrap();
    let wrong = CandidateObservation::new(
        old.id().clone(),
        base.contract(),
        wrong,
        old.basis().cloned(),
    );
    assert_eq!(
        MissionContext::new(base.contract().clone(), Some(wrong)).unwrap_err(),
        ReadinessError::IncompatibleReference
    );
    for capture in [
        CandidateObservation::new(old.id().clone(), base.contract(), old.state().clone(), None),
        CandidateObservation::new(
            old.id().clone(),
            base.contract(),
            RepositoryState::new(
                None,
                Head::Unknown,
                vec![],
                ObservationCoverage::GitStatus,
                ObservationConsistency::TwoMatchingSamples,
            )
            .unwrap(),
            old.basis().cloned(),
        ),
    ] {
        let mut ms = missions();
        ms[0] = Mission::new(
            MissionContext::new(base.contract().clone(), Some(capture)).unwrap(),
            ConsumerKind::Preview,
            DependencyCoverage::DeclaredComplete,
        );
        let mut p = ReadinessProjection::new(
            DependencyGraph::new(graph_ref(1), ms, edges(), limits()).unwrap(),
        )
        .unwrap();
        start_roots(&mut p);
        assert!(
            p.mission(&task("A"))
                .unwrap()
                .blockers()
                .contains(&MissionBlocker::MissingContext)
        );
        let a = report(
            &mut p,
            "a",
            "A",
            artifact("a1", 1, "1"),
            DependencyKind::Artifact,
            ReadinessLevel::IntegratedAccepted,
        );
        release(&mut p, "review", "AB", a, &["pAB"]);
        assert_eq!(
            p.edge(&eid("AB")).unwrap().decision(),
            &EdgeDecision::Held(HoldReason::MissingContext)
        );
    }
}

#[test]
fn integrated_effectful_model_is_advisory_and_capacity_has_exact_neighbors() {
    let mut ms = missions();
    ms[1] = Mission::new(
        context("B", "b", 1),
        ConsumerKind::Effectful,
        DependencyCoverage::DeclaredComplete,
    );
    let mut p = ReadinessProjection::new(
        DependencyGraph::new(graph_ref(1), ms, edges(), limits()).unwrap(),
    )
    .unwrap();
    start_roots(&mut p);
    slots(&mut p, "exact-capacity", 6);
    let n = notification(
        &p,
        "excess-capacity",
        Event::Capacity {
            available: 7,
            source: source("excess"),
        },
    );
    expect_atomic(&mut p, n, ReadinessError::Limit(LimitKind::Missions));
    let a = report(
        &mut p,
        "a",
        "A",
        artifact("a1", 1, "1"),
        DependencyKind::Artifact,
        ReadinessLevel::IntegratedAccepted,
    );
    release(&mut p, "review", "AB", a, &["pAB"]);
    assert!(eligible(&p, "B"));
    assert_eq!(
        p.mission(&task("B")).unwrap().authority(),
        ExecutionAuthority::Unavailable
    );
    ready(&mut p, "ready-A-again", "A");
    let n = notification(
        &p,
        "unmet",
        Event::Preconditions {
            scope: p.scope(&task("A")).unwrap(),
            ready: false,
            source: source("unmet"),
        },
    );
    p.apply(n).unwrap();
    assert!(
        p.mission(&task("A"))
            .unwrap()
            .blockers()
            .contains(&MissionBlocker::PreconditionsUnmet)
    );
    assert!(!eligible(&p, "A"));
}

#[test]
fn invalidation_root_errors_are_atomic_and_first_historical_cause_is_retained() {
    let mut p = prime();
    let a = p.scope(&task("A")).unwrap();
    let n = notification(
        &p,
        "invalid",
        Event::Invalidate {
            roots: vec![a.clone(), a.clone()],
            cause: source("duplicate"),
            coverage: DependencyCoverage::DeclaredComplete,
        },
    );
    expect_atomic(&mut p, n, ReadinessError::IdentityConflict);
    let n = notification(
        &p,
        "invalid",
        Event::Invalidate {
            roots: vec![],
            cause: source("empty"),
            coverage: DependencyCoverage::DeclaredComplete,
        },
    );
    expect_atomic(&mut p, n, ReadinessError::IncompatibleReference);
    apply(
        &mut p,
        "first",
        Event::Invalidate {
            roots: vec![a],
            cause: source("first-cause"),
            coverage: DependencyCoverage::DeclaredComplete,
        },
    );
    let a = p.scope(&task("A")).unwrap();
    apply(
        &mut p,
        "second",
        Event::Invalidate {
            roots: vec![a],
            cause: source("second-cause"),
            coverage: DependencyCoverage::DeclaredComplete,
        },
    );
    assert_eq!(
        p.edge(&eid("AB")).unwrap().releases()[0].invalidated_by(),
        Some(&nid("first"))
    );
    assert_eq!(p.scope(&task("A")).unwrap().generation(), rev(3));
    assert!(released(&p, "DE"));
}
