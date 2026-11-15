//! Scheduling preparation uses real guards and new stores, with no child execution.
#[path = "../../cortex-evidence/tests/support/program.rs"]
mod fixture;
#[path = "../../cortex-evidence/tests/support/operational.rs"]
mod operational_fixture;
use cortex_cli::program_preparation::*;
use cortex_core::{
    binding::CandidateRequirement,
    program::{context::*, domain::PoolIdentity, memory::*, operational::*, scheduling::*, *},
};
use cortex_evidence::program::{Command, CoordinationKernel, KernelError};
use cortex_storage::{
    history_store::{Access, StoreKey},
    program_store::{ProgramStore, ProgramStoreError, ReadLimits},
};
use fixture::*;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

fn kernel() -> CoordinationKernel {
    let mut kernel = initialized();
    operational_fixture::effect(
        &mut kernel,
        "policy",
        EffectEvent::Policy(operational_fixture::policy()),
    );
    kernel
}
fn proposal(id: &str) -> Proposal {
    Proposal {
        task: task(id),
        band: 0,
        waiting_rounds: 0,
        risk_rank: 0,
        cost_rank: 0,
        demand: operational_fixture::demand(),
    }
}
fn mapping(kernel: &CoordinationKernel, id: &str, expected: u64) -> TaskOperations {
    let request = PackageRequest {
        id: PackageId::new(&format!("prepared-package-{id}")).unwrap(),
        revision: rev(1),
        task: task(id),
        baseline: rev(1),
        generation: rev(1),
        sources: vec![],
        methods: vec![],
        omissions: "Explicit synthetic inputs only".into(),
        deliverable: "Metadata proposal".into(),
        stop_rules: "No execution authority or automatic retry".into(),
    };
    let mut claim = operation(
        kernel,
        &format!("prepared-claim-{id}"),
        Command::Claim(attempt(&format!("prepared-attempt-{id}"), id, "operator")),
    );
    claim.expected = expected;
    let mut package = operation(
        kernel,
        &format!("prepared-package-{id}"),
        Command::Package(request),
    );
    package.expected = expected + 1;
    TaskOperations {
        task: task(id),
        candidate: kernel
            .state()
            .readiness()
            .scope(&task(id))
            .unwrap()
            .context()
            .contract()
            .candidate()
            .clone(),
        operations: Some([claim, package]),
    }
}
fn ids(result: &CoordinationPreparation) -> Vec<&str> {
    result
        .operations()
        .iter()
        .map(|operation| operation.id.as_str())
        .collect()
}
fn stale(error: PreparationError) {
    assert_eq!(error, PreparationError::Core(ProgramError::Stale));
}

#[test]
fn eligible_task_preserves_exact_order_and_predecessors() {
    let kernel = kernel();
    let revision = kernel.revision();
    let result = prepare(
        &kernel,
        revision,
        &[proposal("A")],
        &[mapping(&kernel, "A", revision)],
    )
    .unwrap();
    assert_eq!(result.revision(), revision);
    assert_eq!(ids(&result), ["prepared-claim-A", "prepared-package-A"]);
    assert_eq!(
        result
            .operations()
            .iter()
            .map(|op| op.expected)
            .collect::<Vec<_>>(),
        [revision, revision + 1]
    );
    assert!(result.decisions()[0].scheduling.proposed);
    assert!(result.decisions()[0].scheduling.held.is_empty());
    assert_eq!(result.decisions()[0].coordination_hold, None);
}

#[test]
fn missing_dependency_is_held_without_manufacturing_a_release() {
    let kernel = kernel();
    let result = prepare(
        &kernel,
        kernel.revision(),
        &[proposal("B")],
        &[mapping(&kernel, "B", kernel.revision())],
    )
    .unwrap();
    assert!(result.operations().is_empty());
    assert_eq!(
        result.decisions()[0].scheduling.held,
        [WaitReason::Preconditions]
    );
    assert!(!result.decisions()[0].scheduling.dependency_ready);
}

#[test]
fn absent_coordination_authority_is_a_disclosed_hold() {
    let kernel = kernel();
    let mut input = mapping(&kernel, "A", kernel.revision());
    input.operations = None;
    let result = prepare(&kernel, kernel.revision(), &[proposal("A")], &[input]).unwrap();
    assert!(result.operations().is_empty());
    assert!(result.decisions()[0].scheduling.proposed);
    assert_eq!(
        result.decisions()[0].coordination_hold,
        Some(PreparationHold::MissingAuthority)
    );
}

#[test]
fn held_dependency_preserves_valid_unrelated_neighbor() {
    let kernel = kernel();
    // B has greater fanout and precedes D, but does not advance the cursor.
    let inputs = [
        mapping(&kernel, "B", kernel.revision()),
        mapping(&kernel, "D", kernel.revision()),
    ];
    let result = prepare(
        &kernel,
        kernel.revision(),
        &[proposal("D"), proposal("B")],
        &inputs,
    )
    .unwrap();
    assert_eq!(
        result
            .decisions()
            .iter()
            .map(|d| d.scheduling.task.as_str())
            .collect::<Vec<_>>(),
        ["B", "D"]
    );
    assert_eq!(ids(&result), ["prepared-claim-D", "prepared-package-D"]);
}

#[test]
fn exact_two_task_sequence_requires_existing_evidence_backed_release() {
    let mut kernel = kernel();
    let revision = kernel.revision();
    let result = prepare(
        &kernel,
        revision,
        &[proposal("A"), proposal("B")],
        &[
            mapping(&kernel, "A", revision),
            mapping(&kernel, "B", revision),
        ],
    )
    .unwrap();
    // Canonical fanout places held B before A; no dependency release is invented.
    assert_eq!(ids(&result), ["prepared-claim-A", "prepared-package-A"]);
    for operation in result.into_operations() {
        kernel.admit(operation).unwrap();
    }
    release(&mut kernel, "AB");
    let result = prepare(
        &kernel,
        kernel.revision(),
        &[proposal("B")],
        &[mapping(&kernel, "B", kernel.revision())],
    )
    .unwrap();
    assert_eq!(ids(&result), ["prepared-claim-B", "prepared-package-B"]);
    assert!(result.decisions()[0].scheduling.dependency_ready);
    assert_eq!(kernel.state().attempts().len(), 1);
}

#[test]
fn duplicate_task_mapping_is_rejected() {
    let kernel = kernel();
    let input = mapping(&kernel, "A", kernel.revision());
    assert_eq!(
        prepare(
            &kernel,
            kernel.revision(),
            &[proposal("A")],
            &[input.clone(), input]
        )
        .unwrap_err(),
        PreparationError::Mapping
    );
}

#[test]
fn missing_or_foreign_task_mapping_is_rejected() {
    let kernel = kernel();
    for mappings in [vec![], vec![mapping(&kernel, "D", kernel.revision())]] {
        assert_eq!(
            prepare(&kernel, kernel.revision(), &[proposal("A")], &mappings).unwrap_err(),
            PreparationError::Mapping
        );
    }
}

#[test]
fn duplicate_proposal_task_is_rejected() {
    let kernel = kernel();
    assert_eq!(
        prepare(
            &kernel,
            kernel.revision(),
            &[proposal("A"), proposal("A")],
            &[mapping(&kernel, "A", kernel.revision())]
        )
        .unwrap_err(),
        PreparationError::Mapping
    );
}

#[test]
fn stale_selected_and_held_predecessors_are_rejected() {
    let kernel = kernel();
    for id in ["A", "B"] {
        stale(
            prepare(
                &kernel,
                kernel.revision(),
                &[proposal(id)],
                &[mapping(&kernel, id, kernel.revision() - 1)],
            )
            .unwrap_err(),
        );
    }
    stale(
        prepare(
            &kernel,
            kernel.revision() - 1,
            &[proposal("A")],
            &[mapping(&kernel, "A", kernel.revision())],
        )
        .unwrap_err(),
    );
}

#[test]
fn changed_candidate_is_rejected_even_without_sources_or_authority() {
    let kernel = kernel();
    for no_authority in [false, true] {
        let mut input = mapping(&kernel, "A", kernel.revision());
        input.candidate = CandidateRequirement::new(
            input.candidate.repository().clone(),
            candidate("f"),
            input.candidate.worktree(),
        );
        if no_authority {
            input.operations = None;
        }
        stale(prepare(&kernel, kernel.revision(), &[proposal("A")], &[input]).unwrap_err());
    }
}

#[test]
fn changed_attempt_or_package_generation_is_rejected() {
    let kernel = kernel();
    for index in [0, 1] {
        let mut input = mapping(&kernel, "A", kernel.revision());
        match &mut input.operations.as_mut().unwrap()[index].command {
            Command::Claim(attempt) => attempt.generation = rev(2),
            Command::Package(request) => request.generation = rev(2),
            _ => unreachable!(),
        }
        stale(prepare(&kernel, kernel.revision(), &[proposal("A")], &[input]).unwrap_err());
    }
}

#[test]
fn unresolved_effect_retains_its_capacity_and_unknown_outcome() {
    let (mut kernel, scope) = operational_fixture::fixture_kernel();
    let intent = operational_fixture::intent(&scope, "unresolved-preparation-effect");
    operational_fixture::effect(
        &mut kernel,
        "reserve",
        EffectEvent::Reserve {
            intent: intent.clone(),
            at: 2,
        },
    );
    operational_fixture::effect(
        &mut kernel,
        "dispatch",
        EffectEvent::Dispatch {
            effect: intent.id.clone(),
            scope: scope.clone(),
            at: 3,
            basis: source("synthetic-dispatch-only"),
        },
    );
    operational_fixture::effect(
        &mut kernel,
        "unknown",
        EffectEvent::Unknown {
            effect: intent.id,
            scope,
            at: 4,
            basis: source("synthetic-unresolved"),
        },
    );
    let before = kernel.fixture_effects().clone();
    let mut proposed = proposal("D");
    proposed.demand[1].amount = Some(60);
    let result = prepare(
        &kernel,
        kernel.revision(),
        &[proposed],
        &[mapping(&kernel, "D", kernel.revision())],
    )
    .unwrap();
    assert!(result.operations().is_empty());
    assert_eq!(
        result.decisions()[0].scheduling.held,
        [WaitReason::Contention(Resource::OutputBytes)]
    );
    assert!(kernel.fixture_effects() == &before);
    assert_eq!(
        kernel
            .fixture_effects()
            .accounted(Resource::OutputBytes)
            .unwrap(),
        30
    );
    assert_eq!(
        kernel.fixture_effects().effects()[0].status,
        EffectStatus::Unknown
    );
}

#[test]
fn missing_resource_quantity_is_not_zero() {
    let kernel = kernel();
    let mut proposed = proposal("A");
    proposed.demand[1].amount = None;
    let result = prepare(
        &kernel,
        kernel.revision(),
        &[proposed],
        &[mapping(&kernel, "A", kernel.revision())],
    )
    .unwrap();
    assert!(result.operations().is_empty());
    assert_eq!(
        result.decisions()[0].scheduling.held,
        [WaitReason::Unknown(Resource::OutputBytes)]
    );
}

fn memory(pool: &str) -> Resource {
    Resource::memory(
        MemoryBasis::ProcessResident,
        MemoryScope::AccountingPool(PoolIdentity {
            physical: pool.into(),
            incarnation: "1".into(),
            host: "synthetic-host".into(),
            boot: "synthetic-boot".into(),
        }),
    )
    .unwrap()
}

#[test]
fn memory_accounting_pool_identity_must_match() {
    let mut kernel = initialized();
    let mut policy = operational_fixture::policy();
    policy.capacities.push(Capacity {
        resource: memory("pool-a"),
        available: Some(100),
        verification_reserve: 0,
        declared_enforceable: true,
    });
    operational_fixture::effect(&mut kernel, "policy", EffectEvent::Policy(policy));
    let mut valid = proposal("A");
    valid.demand.push(Demand {
        resource: memory("pool-a"),
        amount: Some(5),
    });
    assert_eq!(
        prepare(
            &kernel,
            kernel.revision(),
            &[valid.clone()],
            &[mapping(&kernel, "A", kernel.revision())]
        )
        .unwrap()
        .operations()
        .len(),
        2
    );
    valid.demand[2].resource = memory("pool-b");
    assert!(matches!(
        prepare(
            &kernel,
            kernel.revision(),
            &[valid],
            &[mapping(&kernel, "A", kernel.revision())]
        ),
        Err(PreparationError::Operational(OperationalError::Structure))
    ));
}

#[test]
fn malformed_grant_is_not_an_authority_hold() {
    let kernel = kernel();
    let mut input = mapping(&kernel, "A", kernel.revision());
    let grant = &mut input.operations.as_mut().unwrap()[0].grant;
    grant.id = GrantId::new("invalid-grant").unwrap();
    grant.permissions.clear();
    assert_eq!(
        prepare(&kernel, kernel.revision(), &[proposal("A")], &[input]).unwrap_err(),
        PreparationError::Core(ProgramError::Authority)
    );
}

#[test]
fn release_and_supervised_effect_commands_cannot_be_prepared() {
    let mut kernel = kernel();
    let release = prepare_release(&mut kernel, "AB", true);
    let commands = [
        Command::Release(Box::new(release)),
        Command::SupervisedEffect(Box::new(EffectEvent::Policy(operational_fixture::policy()))),
    ];
    for command in commands {
        let mut input = mapping(&kernel, "A", kernel.revision());
        input.operations.as_mut().unwrap()[1].command = command;
        assert_eq!(
            prepare(&kernel, kernel.revision(), &[proposal("A")], &[input]).unwrap_err(),
            PreparationError::UnsupportedCommand
        );
    }
}

#[test]
fn preparation_leaves_original_kernel_unchanged() {
    let kernel = kernel();
    let before = kernel.clone();
    prepare(
        &kernel,
        kernel.revision(),
        &[proposal("A")],
        &[mapping(&kernel, "A", kernel.revision())],
    )
    .unwrap();
    assert_eq!(kernel.revision(), before.revision());
    assert!(kernel.operations() == before.operations());
    assert!(kernel.state().definitions() == before.state().definitions());
    assert!(kernel.state().attempts() == before.state().attempts());
    assert!(kernel.state().ordering_plan() == before.state().ordering_plan());
    for id in ["A", "B", "C", "D", "E", "F"] {
        assert!(
            kernel.state().readiness().scope(&task(id)).unwrap()
                == before.state().readiness().scope(&task(id)).unwrap()
        );
        assert_eq!(
            kernel
                .state()
                .readiness()
                .task(&task(id))
                .unwrap()
                .eligible(),
            before
                .state()
                .readiness()
                .task(&task(id))
                .unwrap()
                .eligible()
        );
    }
    assert!(kernel.packages() == before.packages());
    assert!(kernel.records() == before.records());
    assert!(kernel.decisions() == before.decisions());
    assert!(kernel.fixture_effects() == before.fixture_effects());
}

#[test]
fn duplicate_new_and_historical_operation_ids_are_rejected() {
    let kernel = kernel();
    for historical in [false, true] {
        let mut input = mapping(&kernel, "A", kernel.revision());
        let pair = input.operations.as_mut().unwrap();
        pair[1].id = if historical {
            kernel.operations()[0].id.clone()
        } else {
            pair[0].id.clone()
        };
        assert_eq!(
            prepare(&kernel, kernel.revision(), &[proposal("A")], &[input]).unwrap_err(),
            PreparationError::Core(ProgramError::IdentityConflict)
        );
    }
}

#[test]
fn package_source_candidate_change_is_rejected() {
    let kernel = kernel();
    let mut input = mapping(&kernel, "A", kernel.revision());
    if let Command::Package(request) = &mut input.operations.as_mut().unwrap()[1].command {
        request.sources.push(SourceInput {
            source: source("changed-source"),
            candidate: binding("A", "f", 1).candidate().clone(),
            text: "Explicit changed input".into(),
        });
    }
    assert_eq!(
        prepare(&kernel, kernel.revision(), &[proposal("A")], &[input]).unwrap_err(),
        PreparationError::Kernel(KernelError::Core(ProgramError::Stale))
    );
}

#[test]
fn oversized_package_is_refused_before_preparation() {
    let kernel = kernel();
    let mut input = mapping(&kernel, "A", kernel.revision());
    if let Command::Package(request) = &mut input.operations.as_mut().unwrap()[1].command {
        request.deliverable = "x".repeat(kernel.state().definition().limits().bytes + 1);
    }
    assert_eq!(
        prepare(&kernel, kernel.revision(), &[proposal("A")], &[input]).unwrap_err(),
        PreparationError::Core(ProgramError::Capacity)
    );
}

static NEXT: AtomicU64 = AtomicU64::new(0);
struct StoreFixture {
    root: PathBuf,
    key: StoreKey,
    store: ProgramStore,
}
impl StoreFixture {
    fn new(kernel: &CoordinationKernel) -> Self {
        let root = std::env::temp_dir().join(format!(
            "cortex-prepare-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let key = StoreKey::new(&[0xc1; 32]).unwrap();
        let store = ProgramStore::create_fixture(
            &root.join("store"),
            &key,
            kernel.initial().clone(),
            ReadLimits::new(16_000_000, Duration::from_secs(20)).unwrap(),
        )
        .unwrap();
        for operation in kernel.operations() {
            store.admit(&key, operation.clone()).unwrap();
        }
        Self { root, key, store }
    }
    fn bytes(&self) -> Vec<(PathBuf, Vec<u8>)> {
        let mut files: Vec<_> = fs::read_dir(self.root.join("store"))
            .unwrap()
            .map(|entry| {
                let path = entry.unwrap().path();
                let bytes = fs::read(&path).unwrap();
                (path, bytes)
            })
            .collect();
        files.sort_by(|a, b| a.0.cmp(&b.0));
        files
    }
}
impl Drop for StoreFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn preparation_leaves_canonical_store_bytes_and_revision_unchanged() {
    let kernel = kernel();
    let fixture = StoreFixture::new(&kernel);
    let before = fixture.bytes();
    prepare(
        &kernel,
        kernel.revision(),
        &[proposal("A")],
        &[mapping(&kernel, "A", kernel.revision())],
    )
    .unwrap();
    assert_eq!(fixture.bytes(), before);
    assert_eq!(
        fixture
            .store
            .load(&fixture.key, Access::ReadOnly)
            .unwrap()
            .revision(),
        kernel.revision()
    );
}

#[test]
fn explicit_later_admission_rejects_stale_prepared_sequence() {
    let kernel = kernel();
    let fixture = StoreFixture::new(&kernel);
    let result = prepare(
        &kernel,
        kernel.revision(),
        &[proposal("A")],
        &[mapping(&kernel, "A", kernel.revision())],
    )
    .unwrap();
    let other = mapping(&kernel, "D", kernel.revision()).operations.unwrap();
    fixture.store.admit(&fixture.key, other[0].clone()).unwrap();
    let before = fixture.bytes();
    let failure = fixture
        .store
        .admit(&fixture.key, result.operations()[0].clone())
        .unwrap_err();
    assert!(matches!(
        failure.error,
        ProgramStoreError::Domain(KernelError::Core(ProgramError::Stale))
    ));
    assert_eq!(fixture.bytes(), before);
    assert_eq!(
        fixture
            .store
            .load(&fixture.key, Access::ReadOnly)
            .unwrap()
            .revision(),
        kernel.revision() + 1
    );
}

#[test]
fn later_failure_preserves_committed_prefix_and_exact_replay() {
    let kernel = kernel();
    let fixture = StoreFixture::new(&kernel);
    let result = prepare(
        &kernel,
        kernel.revision(),
        &[proposal("A")],
        &[mapping(&kernel, "A", kernel.revision())],
    )
    .unwrap();
    let first = fixture
        .store
        .admit(&fixture.key, result.operations()[0].clone())
        .unwrap();
    assert!(!first.duplicate);
    let other = mapping(&kernel, "D", kernel.revision() + 1)
        .operations
        .unwrap();
    fixture.store.admit(&fixture.key, other[0].clone()).unwrap();
    let before = fixture.bytes();
    let failure = fixture
        .store
        .admit(&fixture.key, result.operations()[1].clone())
        .unwrap_err();
    assert!(matches!(
        failure.error,
        ProgramStoreError::Domain(KernelError::Core(ProgramError::Stale))
    ));
    let duplicate = fixture
        .store
        .admit(&fixture.key, result.operations()[0].clone())
        .unwrap();
    assert!(duplicate.duplicate);
    assert_eq!(duplicate.ordinal, first.ordinal);
    assert_eq!(fixture.bytes(), before);
    let retained = fixture.store.load(&fixture.key, Access::ReadOnly).unwrap();
    assert_eq!(retained.revision(), kernel.revision() + 2);
    assert_eq!(retained.state().attempts().len(), 2);
    assert!(retained.packages().is_empty());
    assert!(retained.fixture_effects().effects().is_empty());
}
