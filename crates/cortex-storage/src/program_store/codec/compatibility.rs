//! Synthetic retained-format cases specified from the original positional codec.
use super::*;
use crate::program_store::tests::fixture;
use cortex_core::program::{domain::Consumption, operational::EffectScope, scheduling::*};

fn roundtrip<T: Wire + PartialEq>(expected: T, bytes: &[u8]) {
    let value: V = serde_json::from_slice(bytes).unwrap();
    let decoded = T::read(&value).unwrap();
    assert!(decoded == expected);
    assert_eq!(
        encoded(&expected.write().unwrap(), 1_000_000)
            .unwrap()
            .as_slice(),
        bytes
    );
    assert_eq!(
        encoded(&decoded.write().unwrap(), 1_000_000)
            .unwrap()
            .as_slice(),
        bytes
    );
}

#[test]
fn retained_resource_dependency_and_consumption_tokens_are_not_rust_spellings() {
    for (value, old, renamed) in [
        (Resource::WorkerSlots, "Workers", "WorkerSlots"),
        (Resource::BuildJobs, "Builds", "BuildJobs"),
        (Resource::TestProcesses, "Tests", "TestProcesses"),
        (Resource::CpuMillicores, "CpuMillis", "CpuMillicores"),
        (Resource::ElapsedMillis, "TimeMillis", "ElapsedMillis"),
    ] {
        roundtrip(value, format!("\"{old}\"").as_bytes());
        assert!(Resource::read(&json!(renamed)).is_err());
    }
    for (value, token) in [
        (Resource::RamBytes, "RamBytes"),
        (Resource::DiskBytes, "DiskBytes"),
        (Resource::OutputBytes, "OutputBytes"),
        (Resource::CiJobs, "CiJobs"),
        (Resource::ModelTokens, "ModelTokens"),
    ] {
        roundtrip(value, format!("\"{token}\"").as_bytes());
    }
    roundtrip(DependencyKind::TaskAcceptance, br#""MissionAcceptance""#);
    assert!(DependencyKind::read(&json!("TaskAcceptance")).is_err());
    roundtrip(Consumption::CpuExecutionMillis, br#""CpuExecutionMillis""#);
    roundtrip(Consumption::ElapsedMillis, br#""ElapsedMillis""#);
    assert!(Consumption::read(&json!("CpuMillis")).is_err());
    assert!(Resource::read(&json!("CpuExecutionMillis")).is_err());
    assert!(Resource::read(&V::Null).is_err());
}

#[test]
fn renamed_fields_preserve_positional_meaning_and_arity() {
    roundtrip(
        Capacity {
            resource: Resource::WorkerSlots,
            available: Some(17),
            verification_reserve: 3,
            declared_enforceable: false,
        },
        br#"["Workers",17,3,false]"#,
    );
    roundtrip(
        Demand {
            resource: Resource::CpuMillicores,
            amount: Some(1250),
        },
        br#"["CpuMillis",1250]"#,
    );
    roundtrip(
        Workstream {
            id: WorkstreamId::new("stream").unwrap(),
            responsibility: "Finite responsibility".into(),
            tasks: vec![TaskId::new("A").unwrap(), TaskId::new("B").unwrap()],
        },
        br#"["stream","Finite responsibility",["A","B"]]"#,
    );
    roundtrip(
        Increment {
            id: IncrementId::new("increment").unwrap(),
            task: TaskId::new("A").unwrap(),
            artifacts: vec![],
            claim_scopes: vec![ClaimScopeId::new("shared-contract").unwrap()],
        },
        br#"["increment","A",[],["shared-contract"]]"#,
    );
    let scope = EffectScope {
        program: ProgramId::new("program").unwrap(),
        baseline: ContractRevision::new(2).unwrap(),
        task: TaskId::new("A").unwrap(),
        candidate: fixture::binding("A", "a", 1).candidate().clone(),
        attempt: AttemptId::new("attempt").unwrap(),
        generation: ContractRevision::new(3).unwrap(),
        package: PackageId::new("package").unwrap(),
        package_revision: ContractRevision::new(4).unwrap(),
    };
    roundtrip(
        scope,
        br#"["program",2,"A",[[[47,115,121,110,116,104,101,116,105,99,47,99,111,114,116,101,120,45,112,114,111,103,114,97,109,45,65],[47,115,121,110,116,104,101,116,105,99,47,99,111,114,116,101,120,45,112,114,111,103,114,97,109,45,65,47,46,103,105,116],[47,115,121,110,116,104,101,116,105,99,47,99,111,114,116,101,120,45,112,114,111,103,114,97,109,45,65,47,46,103,105,116]],"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","Clean"],"attempt",3,"package",4]"#,
    );
    let request = PackageRequest {
        id: PackageId::new("package").unwrap(),
        revision: ContractRevision::new(2).unwrap(),
        task: TaskId::new("A").unwrap(),
        baseline: ContractRevision::new(3).unwrap(),
        generation: ContractRevision::new(4).unwrap(),
        sources: vec![],
        methods: vec![],
        omissions: "Missing input".into(),
        deliverable: "Finite artifact".into(),
        stop_rules: "No dispatch".into(),
    };
    roundtrip(
        request,
        br#"["package",2,"A",3,4,[],[],"Missing input","Finite artifact","No dispatch"]"#,
    );
    for bad in [
        json!(["Workers", 17, 3]),
        json!(["Workers", 17, 3, false, 0]),
        json!(["WorkerSlots", 17, 3, false]),
    ] {
        assert!(Capacity::read(&bad).is_err());
    }
    // Same-typed permutations can be valid records; the golden encoder oracle,
    // not an invented validation rule, protects which value occupies each slot.
    let swapped: Capacity = read(&json!(["Workers", 3, 17, false])).unwrap();
    assert_eq!(swapped.available, Some(3));
    assert_eq!(swapped.verification_reserve, 17);
}

#[test]
fn legacy_ordering_operation_retains_identity_and_canonical_bytes() {
    use fixture::{definition, grant, rev, source, task};
    let mut kernel = CoordinationKernel::new(definition()).unwrap();
    let expected = Operation {
        id: OperationId::new("order-1").unwrap(),
        expected: 0,
        grant: grant(1, "operator"),
        command: Command::OrderingPlan(OrderingPlan {
            revision: rev(1),
            order: ["A", "B", "C", "D", "E", "F"]
                .into_iter()
                .map(task)
                .collect(),
            basis: source("order-basis"),
        }),
    };
    // Original v1 field order, original command tag and literal caller IDs.
    const GOLDEN: &[u8] = br#"["order-1",0,["grant-operator-1","fixture-program",1,"operator",["A","B","C","D","E","F"],["Amend","Plan","Claim","Review"],[["operator","fixture-operator"],"explicit-local-operator-metadata","1"]],["plan",[1,["A","B","C","D","E","F"],[["operator","fixture-operator"],"order-basis","1"]]]]"#;
    let decoded = decode_operation(GOLDEN, &kernel).unwrap();
    assert!(decoded == expected);
    assert_eq!(operation(&expected, 1_000_000).unwrap().as_slice(), GOLDEN);
    assert_eq!(operation(&decoded, 1_000_000).unwrap().as_slice(), GOLDEN);
    assert_eq!(decoded.id.as_str(), "order-1");
    kernel.admit(decoded.clone()).unwrap();
    kernel.admit(decoded).unwrap();
    assert_eq!(kernel.revision(), 1);
    assert_eq!(kernel.operations().len(), 1);
    let recovered = decode_operation(GOLDEN, &kernel).unwrap();
    assert!(recovered == expected);
    assert_eq!(kernel.revision(), 1); // Decoding itself does not admit or dispatch.
    let renamed = String::from_utf8(GOLDEN.to_vec())
        .unwrap()
        .replace("\"plan\"", "\"ordering-plan\"");
    assert!(decode_operation(renamed.as_bytes(), &kernel).is_err());
    let mut spaced = GOLDEN.to_vec();
    spaced.push(b' ');
    assert!(decode_operation(&spaced, &kernel).is_err());
    assert!(decode_operation(br#"["order-1",0]"#, &kernel).is_err());
}

#[test]
fn task_context_scope_graph_and_claim_ids_keep_the_retained_structure() {
    use fixture::{binding, rev, task};
    let context = TaskContext::new(binding("A", "a", 1), None).unwrap();
    // The original context encoded the binding then an optional capture. The
    // declared identity text remains A; renaming its wrapper adds no identity.
    let expected_context = json!([
        [
            "A",
            "Retain intermediate delivery while producer continues",
            "Exact declared interface behavior",
            1,
            [
                [
                    b"/synthetic/cortex-program-A",
                    b"/synthetic/cortex-program-A/.git",
                    b"/synthetic/cortex-program-A/.git"
                ],
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "Clean"
            ]
        ],
        null
    ]);
    roundtrip(
        context.clone(),
        &serde_json::to_vec(&expected_context).unwrap(),
    );
    roundtrip(
        TaskScope::new(context.clone(), rev(7)),
        &serde_json::to_vec(&json!([expected_context, 7])).unwrap(),
    );
    let coordinated = Task::new(
        context,
        ConsumerKind::Preview,
        DependencyCoverage::DeclaredComplete,
    );
    let expected_task = json!([expected_context, "Preview", "DeclaredComplete"]);
    roundtrip(
        coordinated.clone(),
        &serde_json::to_vec(&expected_task).unwrap(),
    );
    let graph = DependencyGraph::new(
        GraphReference::new(GraphId::new("graph").unwrap(), rev(2)),
        vec![coordinated],
        vec![],
        GraphLimits::new(3, 4, 5, 64, 16384).unwrap(),
    )
    .unwrap();
    roundtrip(
        graph,
        &serde_json::to_vec(&json!([
            ["graph", 2],
            [expected_task],
            [],
            [3, 4, 5, 64, 16384]
        ]))
        .unwrap(),
    );
    roundtrip(
        NonImpact {
            tasks: vec![task("A")],
            basis: fixture::source("unchanged"),
        },
        br#"[["A"],[["operator","fixture-operator"],"unchanged","1"]]"#,
    );
    roundtrip(
        ClaimScopeId::new("semantic.contract").unwrap(),
        br#""semantic.contract""#,
    );
    assert!(ClaimScopeId::read(&json!("")).is_err());
    assert!(TaskScope::read(&json!([expected_context, 0])).is_err());
}

#[test]
fn retained_signal_position_and_constructor_rejection_are_independent_of_rust_names() {
    const GOLDEN: &[u8] = br#"["signal-id",["graph",2],["signal","A","Public synthetic message"]]"#;
    let expected = Notification::new(
        NotificationId::new("signal-id").unwrap(),
        GraphReference::new(GraphId::new("graph").unwrap(), fixture::rev(2)),
        Event::Signal {
            task: fixture::task("A"),
            message: "Public synthetic message".into(),
        },
    )
    .unwrap();
    roundtrip(expected, GOLDEN);
    for invalid in [
        json!(["signal-id", ["graph", 2], ["signal", "A"]]),
        json!([
            "signal-id",
            ["graph", 2],
            ["signal", "A", "message", "extra"]
        ]),
        json!(["signal-id", ["graph", 0], ["signal", "A", "message"]]),
        json!(["signal-id", ["graph", 2], ["signal", "", "message"]]),
        json!(["signal-id", ["graph", 2], ["task-signal", "A", "message"]]),
    ] {
        assert!(Notification::read(&invalid).is_err());
    }
}
