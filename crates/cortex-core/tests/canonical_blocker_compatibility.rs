//! Independent retained-package counterexamples for every task-blocker constructor.
#[path = "support/program.rs"]
mod fixture;
use cortex_core::{
    parallel::*,
    program::{context::*, *},
};
use fixture::*;

fn fields(bytes: &[u8]) -> Vec<(&str, &[u8])> {
    let mut fields = Vec::new();
    let mut remaining = bytes;
    while !remaining.is_empty() {
        let newline = remaining.iter().position(|b| *b == b'\n').unwrap();
        let header = std::str::from_utf8(&remaining[..newline]).unwrap();
        let (name, count) = header.split_once(' ').unwrap();
        let count = count.parse::<usize>().unwrap();
        remaining = &remaining[newline + 1..];
        let (value, rest) = remaining.split_at(count);
        assert_eq!(rest.first(), Some(&b'\n'));
        fields.push((name, value));
        remaining = &rest[1..];
    }
    fields
}

fn assert_retained_blockers(
    state: &ProgramState,
    id: &str,
    expected: &[TaskBlocker],
    retained: &[(&str, &[u8])],
) {
    assert_eq!(
        state.readiness().task(&task(id)).unwrap().blockers(),
        expected
    );
    let request = PackageRequest {
        id: PackageId::new("adversarial-package").unwrap(),
        revision: rev(1),
        task: task(id),
        baseline: rev(1),
        generation: rev(1),
        sources: vec![],
        methods: vec![],
        omissions: "Synthetic context only".into(),
        deliverable: "Retain old package bytes".into(),
        stop_rules: "No execution".into(),
    };
    let package = assemble(state, request).unwrap();
    let decoded = fields(package.bytes());
    let actual: Vec<_> = decoded
        .iter()
        .copied()
        .filter(|(name, _)| matches!(*name, "blocker" | "blocked-edge"))
        .collect();
    assert_eq!(actual, retained);
    assert!(decoded.contains(&("mission", id.as_bytes())));
    assert!(!decoded.iter().any(|(name, _)| *name == "task"));
    assert!(package.check(state, package.bytes()).is_ok());
    let changed = String::from_utf8(package.bytes().to_vec())
        .unwrap()
        .replace("MissionBlocker", "TaskBlocker");
    assert_eq!(
        package.check(state, changed.as_bytes()),
        Err(ProgramError::Stale)
    );
}

#[test]
fn all_blocker_constructors_keep_opaque_legacy_bytes_and_dependency_identity() {
    // The base redacted! macro printed this exact constant for every non-edge
    // MissionBlocker variant. Dependency was the assembler's explicit exception.
    const OLD: &[u8] = b"MissionBlocker([redacted])";
    let initial = ProgramState::new(definition()).unwrap();
    assert_retained_blockers(
        &initial,
        "A",
        &[
            TaskBlocker::PreconditionsUnknown,
            TaskBlocker::CapacityUnknown,
        ],
        &[("blocker", OLD), ("blocker", OLD)],
    );
    assert_retained_blockers(
        &initial,
        "B",
        &[
            TaskBlocker::PreconditionsUnknown,
            TaskBlocker::Dependency(EdgeId::new("AB").unwrap()),
            TaskBlocker::CapacityUnknown,
        ],
        &[("blocker", OLD), ("blocked-edge", b"AB"), ("blocker", OLD)],
    );

    let mut active = ProgramState::new(definition()).unwrap();
    initialize(&mut active);
    active
        .claim(&grant(1, "operator"), attempt("active-A", "A", "operator"))
        .unwrap();
    assert_retained_blockers(
        &active,
        "A",
        &[TaskBlocker::NotPending],
        &[("blocker", OLD)],
    );

    let original = definition();
    let graph = DependencyGraph::new(
        original.graph().reference().clone(),
        original
            .graph()
            .tasks()
            .iter()
            .map(|t| {
                Task::new(
                    TaskContext::new(t.context().contract().clone(), None).unwrap(),
                    t.kind(),
                    DependencyCoverage::Unknown,
                )
            })
            .collect(),
        original.graph().edges().to_vec(),
        original.graph().limits(),
    )
    .unwrap();
    let incomplete = ProgramDefinition::new(
        original.id().clone(),
        original.outcome().clone(),
        original.revision(),
        original.predecessor(),
        original.approval().clone(),
        graph,
        original.workstreams().to_vec(),
        original.increments().to_vec(),
        original.limits(),
    )
    .unwrap();
    let mut blocked = ProgramState::new(incomplete).unwrap();
    let g = grant(1, "operator");
    let precondition = notify(
        &blocked,
        "unmet",
        Event::Preconditions {
            scope: blocked.readiness().scope(&task("A")).unwrap(),
            ready: false,
            source: source("unmet"),
        },
    );
    blocked.notify(&g, precondition).unwrap();
    let capacity = notify(
        &blocked,
        "no-capacity",
        Event::Capacity {
            available: 0,
            source: source("no-capacity"),
        },
    );
    blocked.notify(&g, capacity).unwrap();
    assert_retained_blockers(
        &blocked,
        "A",
        &[
            TaskBlocker::PreconditionsUnmet,
            TaskBlocker::MissingContext,
            TaskBlocker::UnknownCoverage,
            TaskBlocker::NoCapacity,
        ],
        &[
            ("blocker", OLD),
            ("blocker", OLD),
            ("blocker", OLD),
            ("blocker", OLD),
        ],
    );
}
