#![allow(dead_code)]
use cortex_core::{binding::*, parallel::*, program::*, temporal::SourceReference, *};
use std::path::PathBuf;
pub fn rev(n: u64) -> ContractRevision {
    ContractRevision::new(n).unwrap()
}
pub fn task(s: &str) -> TaskId {
    TaskId::new(s).unwrap()
}
pub fn source(s: &str) -> SourceReference {
    SourceReference::operator_reported("fixture-operator", s, "1").unwrap()
}
pub fn candidate(s: &str) -> CandidateId {
    CandidateId::from_git_commit(&s.repeat(40)).unwrap()
}
pub fn binding(id: &str, hex: &str, n: u64) -> CandidateBindingContract {
    let root = PathBuf::from(format!("/synthetic/cortex-program-{id}"));
    CandidateBindingContract::new(
        TaskContract::new(
            task(id),
            "Retain intermediate delivery while producer continues",
            "Exact declared interface behavior",
        )
        .unwrap(),
        rev(n),
        CandidateRequirement::new(
            RepositoryIdentity::new(root.clone(), root.join(".git"), root.join(".git")).unwrap(),
            candidate(hex),
            WorktreeRequirement::Clean,
        ),
    )
}
pub fn capture(c: &CandidateBindingContract, n: u64) -> CandidateObservation {
    CandidateObservation::new(
        ObservationId::new(c.task().id().as_str(), n).unwrap(),
        c,
        RepositoryState::new(
            Some(c.candidate().repository().clone()),
            Head::Detached {
                commit: c.candidate().commit().clone(),
            },
            vec![],
            ObservationCoverage::GitStatus,
            ObservationConsistency::TwoMatchingSamples,
        )
        .unwrap(),
        Some(ComparisonBasis::new("synthetic-fixture", b"exact-fixture".to_vec()).unwrap()),
    )
}
pub fn definition() -> ProgramDefinition {
    definition_revision(1, false)
}
pub fn with_contracts(
    definition: &ProgramDefinition,
    replace: impl Fn(&CandidateBindingContract) -> CandidateBindingContract,
) -> ProgramDefinition {
    let tasks = definition
        .graph()
        .tasks()
        .iter()
        .map(|task| {
            let contract = replace(task.context().contract());
            Task::new(
                TaskContext::new(contract.clone(), Some(capture(&contract, 1))).unwrap(),
                task.kind(),
                task.coverage(),
            )
        })
        .collect();
    let graph = DependencyGraph::new(
        definition.graph().reference().clone(),
        tasks,
        definition.graph().edges().to_vec(),
        definition.graph().limits(),
    )
    .unwrap();
    ProgramDefinition::new(
        definition.id().clone(),
        definition.outcome().clone(),
        definition.revision(),
        definition.predecessor(),
        definition.approval().clone(),
        graph,
        definition.workstreams().to_vec(),
        definition.increments().to_vec(),
        definition.limits(),
    )
    .unwrap()
}
pub fn definition_revision(n: u64, change_a: bool) -> ProgramDefinition {
    let tasks: Vec<_> = [
        ("A", "a"),
        ("B", "b"),
        ("C", "c"),
        ("D", "d"),
        ("E", "e"),
        ("F", "f"),
    ]
    .into_iter()
    .map(|(id, hex)| {
        let c = binding(id, hex, if id == "A" && change_a { n } else { 1 });
        Task::new(
            TaskContext::new(c.clone(), Some(capture(&c, 1))).unwrap(),
            ConsumerKind::Preview,
            DependencyCoverage::DeclaredComplete,
        )
    })
    .collect();
    let edges: Vec<_> = [
        ("AB", "A", "B", "a"),
        ("BC", "B", "C", "b"),
        ("DE", "D", "E", "d"),
        ("BF", "B", "F", "b"),
        ("EF", "E", "F", "e"),
    ]
    .into_iter()
    .map(|(id, p, c, hex)| {
        DependencyEdge::new(
            EdgeId::new(id).unwrap(),
            task(p),
            task(c),
            ArtifactRequirement::new(
                ArtifactReference::new(ArtifactId::new(p).unwrap(), rev(1), candidate(hex)),
                DependencyKind::Artifact,
                ReadinessLevel::ContractStable,
                vec![PredicateId::new(id).unwrap()],
            )
            .unwrap(),
        )
    })
    .collect();
    let streams = vec![
        Workstream {
            id: WorkstreamId::new("contracts").unwrap(),
            responsibility: "Contract and validation".into(),
            tasks: vec![task("A"), task("B"), task("C")],
        },
        Workstream {
            id: WorkstreamId::new("context").unwrap(),
            responsibility: "Context and client".into(),
            tasks: vec![task("D"), task("E"), task("F")],
        },
    ];
    let mut increments: Vec<_> = tasks
        .iter()
        .map(|m| Increment {
            id: IncrementId::new(m.id().as_str()).unwrap(),
            task: m.id().clone(),
            artifacts: edges
                .iter()
                .find(|e| e.producer() == m.id())
                .map(|e| vec![e.requirement().artifact().clone()])
                .unwrap_or_default(),
            claim_scopes: vec![ClaimScopeId::new(m.id().as_str()).unwrap()],
        })
        .collect();
    increments.push(Increment {
        id: IncrementId::new("A-later").unwrap(),
        task: task("A"),
        artifacts: vec![],
        claim_scopes: vec![ClaimScopeId::new("A").unwrap()],
    });
    let graph = DependencyGraph::new(
        GraphReference::new(GraphId::new("program-graph").unwrap(), rev(n)),
        tasks,
        edges,
        GraphLimits::new(16, 32, 512, 8192, 8_000_000).unwrap(),
    )
    .unwrap();
    ProgramDefinition::new(
        ProgramId::new("fixture-program").unwrap(),
        ProgramOutcome::new(
            "Coordinate the connected fixture program",
            "Retain accepted scope, useful releases and unresolved work across restart",
        )
        .unwrap(),
        rev(n),
        if n == 1 { None } else { Some(rev(n - 1)) },
        source("accepted-baseline"),
        graph,
        streams,
        increments,
        ProgramLimits::new(8, 512, 16_000_000).unwrap(),
    )
    .unwrap()
}
pub fn grant(n: u64, actor: &str) -> CoordinationGrant {
    CoordinationGrant {
        id: GrantId::new(&format!("grant-{actor}-{n}")).unwrap(),
        program: ProgramId::new("fixture-program").unwrap(),
        baseline: rev(n),
        actor: ActorId::new(actor).unwrap(),
        tasks: ["A", "B", "C", "D", "E", "F"]
            .into_iter()
            .map(task)
            .collect(),
        permissions: vec![
            CoordinationPermission::Amend,
            CoordinationPermission::Plan,
            CoordinationPermission::Claim,
            CoordinationPermission::Review,
        ],
        basis: source("explicit-local-operator-metadata"),
    }
}
pub fn attempt(id: &str, increment: &str, owner: &str) -> Attempt {
    Attempt {
        id: AttemptId::new(id).unwrap(),
        increment: IncrementId::new(increment).unwrap(),
        owner: ActorId::new(owner).unwrap(),
        baseline: rev(1),
        generation: rev(1),
        status: AttemptStatus::Claimed,
        basis: source("claim"),
    }
}
pub fn notify(s: &ProgramState, id: &str, event: Event) -> Notification {
    Notification::new(
        NotificationId::new(id).unwrap(),
        s.definition().graph().reference().clone(),
        event,
    )
    .unwrap()
}
pub fn initialize(s: &mut ProgramState) {
    let g = grant(1, "operator");
    for id in ["A", "B", "C", "D", "E", "F"] {
        s.notify(
            &g,
            notify(
                s,
                &format!("ready-{id}"),
                Event::Preconditions {
                    scope: s.readiness().scope(&task(id)).unwrap(),
                    ready: true,
                    source: source("conditions"),
                },
            ),
        )
        .unwrap();
    }
    s.notify(
        &g,
        notify(
            s,
            "capacity",
            Event::Capacity {
                available: 2,
                source: source("simulated-capacity"),
            },
        ),
    )
    .unwrap();
}
