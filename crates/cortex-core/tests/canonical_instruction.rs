//! Synthetic retained-format oracle, specified from the original v1 assembler.
#[path = "support/program.rs"]
mod fixture;
use cortex_core::{
    parallel::TaskBlocker,
    program::{context::*, *},
};
use fixture::*;

fn request() -> PackageRequest {
    PackageRequest {
        id: PackageId::new("canonical-A").unwrap(),
        revision: rev(2),
        task: task("A"),
        baseline: rev(1),
        generation: rev(1),
        sources: vec![SourceInput {
            source: source("synthetic-input"),
            candidate: binding("A", "a", 1).candidate().clone(),
            text: "Public fixture\nauthority NONE".into(),
        }],
        methods: vec![MethodProposal {
            method: source("fixture-method"),
            applicability: "Inspect a finite synthetic package".into(),
            qualification: None,
            exceptions: "None".into(),
        }],
        omissions: "No live observation".into(),
        deliverable: "A bounded contract".into(),
        stop_rules: "Stop on missing evidence".into(),
    }
}

#[test]
fn renamed_task_preserves_exact_v1_package_and_opaque_blocker_bytes() {
    let mut state = ProgramState::new(definition()).unwrap();
    state
        .set_ordering_plan(
            &grant(1, "operator"),
            OrderingPlan {
                revision: rev(1),
                order: ["A", "B", "C", "D", "E", "F"]
                    .into_iter()
                    .map(task)
                    .collect(),
                basis: source("ordering"),
            },
        )
        .unwrap();
    let projected = state.readiness().task(&task("A")).unwrap();
    assert!(matches!(
        projected.blockers(),
        [
            TaskBlocker::PreconditionsUnknown,
            TaskBlocker::CapacityUnknown
        ]
    ));
    assert!(projected.dependency_ready());
    assert!(!projected.eligible());
    let package = assemble(&state, request()).unwrap();
    // This fixture was transcribed from the original field order, framing,
    // literals and declared inputs, independently of the renamed implementation.
    let original = include_bytes!("fixtures/instruction-v1-golden.txt");
    assert_eq!(package.bytes(), original);
    assert!(package.check(&state, original).is_ok());
    assert_eq!(package.request().id.as_str(), "canonical-A");
    let renamed_wire = String::from_utf8(original.to_vec())
        .unwrap()
        .replace("MissionBlocker", "TaskBlocker");
    assert_eq!(
        package.check(&state, renamed_wire.as_bytes()),
        Err(ProgramError::Stale)
    );
    let mut changed_identity = original.to_vec();
    let start = changed_identity
        .windows(b"canonical-A".len())
        .position(|w| w == b"canonical-A")
        .unwrap();
    changed_identity[start] = b'X';
    assert_eq!(
        package.check(&state, &changed_identity),
        Err(ProgramError::Stale)
    );
}
