use super::*;
use cortex_storage::{
    history_store::Completion,
    program_store::{ReadLimits, Reconciliation},
};
use std::{
    fs::{self, DirBuilder},
    os::unix::fs::DirBuilderExt,
    sync::atomic::AtomicUsize,
};

#[path = "../../../cortex-evidence/tests/support/program.rs"]
mod fixture;
#[path = "../../../cortex-evidence/tests/support/supervision.rs"]
mod supervision;

struct TestStoreRoot(PathBuf);
impl TestStoreRoot {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "cortex-coordinator-ack-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        DirBuilder::new().mode(0o700).create(&path).unwrap();
        Self(path)
    }
}
impl Drop for TestStoreRoot {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn selected_work(
    canonical: &CoordinationKernel,
    scope: &cortex_core::program::operational::EffectScope,
    spec: ProcessSpec,
) -> FixtureWork {
    let id = |name| OperationId::new(name).unwrap();
    let command = |name| {
        &canonical
            .operations()
            .iter()
            .find(|op| op.id == id(name))
            .unwrap()
            .command
    };
    let Command::Claim(attempt) = command("claim-A") else {
        panic!("expected canonical claim");
    };
    let Command::Package(package) = command("package-A") else {
        panic!("expected canonical package");
    };
    let Command::PrepareExecutorInput(input) = command("prepared") else {
        panic!("expected canonical executor preparation");
    };
    FixtureWork {
        proposal: Proposal {
            task: scope.task.clone(),
            band: 0,
            waiting_rounds: 0,
            risk_rank: 0,
            cost_rank: 0,
            demand: supervision::demand(),
        },
        attempt: attempt.clone(),
        package: package.clone(),
        input: input.as_ref().clone(),
        authority: fixture::grant(scope.baseline.get(), "operator"),
        grant: supervision::proposed_grant(scope),
        effect: supervision::intent(scope).id,
        process: spec,
        workspace: None,
        operations: FixtureOperationIds {
            claim: id("claim-A"),
            package: id("package-A"),
            input: id("prepared"),
            grant: id("supervised-grant"),
            reserve: id("reserve"),
            process: id("process-spec"),
            dispatch: id("dispatch"),
        },
        captures: CaptureIds {
            started_operation: id("started"),
            finished_operation: id("finished"),
            started_observation: ObservationId::new("prefix-test", 1).unwrap(),
            finished_observation: ObservationId::new("prefix-test", 2).unwrap(),
            basis: fixture::source("prefix-test"),
            admission_attempts: 1,
        },
        workspace_observation: ObservationId::new("prefix-candidate", 1).unwrap(),
        dispatch_basis: fixture::source("unused-test-dispatch"),
    }
}

#[test]
fn lost_prefix_acknowledgment_requires_exact_readback_and_holds_reopened_work() {
    let (canonical, scope, spec) = supervision::prepared_kernel(None);
    let operations = canonical.operations();
    let start = operations
        .iter()
        .position(|op| op.id.as_str() == "claim-A")
        .unwrap();
    let prefix = &operations[start..];
    let key = StoreKey::new(&[0x4d; 32]).unwrap();
    let limits = ReadLimits::new(8_000_000, Duration::from_secs(10)).unwrap();
    for lost in ["claim-A", "prepared", "reserve"] {
        let scratch = TestStoreRoot::new();
        let path = scratch.0.join("store");
        let store = ProgramStore::create_supervised_fixture(
            &path,
            &key,
            canonical.initial().clone(),
            limits,
        )
        .unwrap();
        for op in &operations[..start] {
            store.admit(&key, op.clone()).unwrap();
        }
        let cut = prefix.iter().position(|op| op.id.as_str() == lost).unwrap();
        let mut receipts = Vec::new();
        let mut offered = Vec::new();
        let outcome = admit_fixture_prefix(
            prefix.to_vec(),
            &mut receipts,
            |op| {
                offered.push(op.id.clone());
                Ok(())
            },
            |op| {
                let suppress = op.id.as_str() == lost;
                let receipt = store.admit(&key, op)?;
                assert!(!receipt.duplicate);
                // The encrypted commit is real. Withhold its successful receipt
                // from the wrapper to model a lost acknowledgment at delivery.
                if suppress {
                    return Err(AdmissionFailure {
                        completion: Completion::Indeterminate,
                        error: ProgramStoreError::Indeterminate,
                    });
                }
                Ok(receipt)
            },
        );
        let Err(FixtureProgress::Admission { operation, failure }) = outcome else {
            panic!("expected the original uncertain operation");
        };
        assert!(*operation == prefix[cut]);
        assert_eq!(failure.completion, Completion::Indeterminate);
        assert!(matches!(failure.error, ProgramStoreError::Indeterminate));
        assert_eq!(receipts.len(), cut);
        assert_eq!(
            offered,
            prefix[..=cut]
                .iter()
                .map(|op| op.id.clone())
                .collect::<Vec<_>>(),
        );

        let store_id = store.id();
        drop(store);
        let reopened = ProgramStore::open(&path, &key, Access::ReadOnly, limits).unwrap();
        assert_eq!(
            reopened
                .reconcile(&key, canonical.initial(), &operation, Access::ReadOnly)
                .unwrap(),
            Reconciliation::Present(AdmissionReceipt {
                store: store_id,
                ordinal: operation.expected + 1,
                duplicate: true,
            }),
        );
        let saved = reopened.load(&key, Access::ReadOnly).unwrap();
        assert!(saved.operations() == &operations[..=start + cut]);
        let effects = saved.fixture_effects().effects();
        if lost == "reserve" {
            assert_eq!(effects.len(), 1);
            assert_eq!(
                effects[0].status,
                cortex_core::program::operational::EffectStatus::Reserved
            );
            assert!(effects[0].dispatch_at.is_none());
            assert!(effects[0].process_observations.is_empty());
        } else {
            assert!(effects.is_empty());
        }
        let turn = run_fixture_turn(
            &reopened,
            &key,
            &[selected_work(&canonical, &scope, spec.clone())],
            None,
            Instant::now() + Duration::from_secs(60),
            &AtomicBool::new(false),
        )
        .unwrap();
        assert!(matches!(&turn.held[..], [FixtureHold::Retained(task)] if task == &scope.task));
        assert!(turn.selected.is_none() && turn.progress.is_none() && turn.receipts.is_empty());
        let unchanged = reopened.load(&key, Access::ReadOnly).unwrap();
        assert!(unchanged.operations() == saved.operations());
    }
}

#[test]
fn real_dispatch_refusal_keeps_exact_input_at_cli_error_boundary() {
    let scratch = TestStoreRoot::new();
    let key = StoreKey::new(&[0x4d; 32]).unwrap();
    let limits = ReadLimits::new(8_000_000, Duration::from_secs(2)).unwrap();
    let (kernel, scope, _) = supervision::prepared_kernel(None);
    let path = scratch.0.join("store");
    let store =
        ProgramStore::create_supervised_fixture(&path, &key, kernel.initial().clone(), limits)
            .unwrap();
    for operation in kernel.operations() {
        store.admit(&key, operation.clone()).unwrap();
    }
    let mut attempted = supervision::dispatch_operation(&kernel, &scope);
    attempted.expected -= 1;
    let failure = store
        .dispatch_fixture(&key, attempted.clone(), |_| {
            panic!("the stale dispatch reached the effect callback");
        })
        .unwrap_err();
    assert_eq!(failure.completion, Completion::Rejected);
    let selected = store.id();
    let error = crate::program_execution::preserve_supervisor_failure(
        selected,
        attempted.clone(),
        HarnessError::Admission(failure),
    );
    let ExecutionError::DispatchAdmission(recovery) = error else {
        panic!("the CLI discarded the canonical failure");
    };
    assert_eq!(recovery.store, selected);
    assert!(recovery.operation == attempted);
    assert_eq!(recovery.failure.completion, Completion::Rejected);
    assert!(matches!(
        recovery.failure.error,
        ProgramStoreError::Domain(KernelError::Core(cortex_core::program::ProgramError::Stale))
    ));
    drop(store);
    let reopened = ProgramStore::open(&path, &key, Access::ReadOnly, limits).unwrap();
    assert_eq!(
        recovery
            .reconcile(&reopened, &key, kernel.initial())
            .unwrap(),
        Reconciliation::Conflict,
    );
    let image = reopened.load(&key, Access::ReadOnly).unwrap();
    assert_eq!(image.revision(), kernel.revision());
    assert!(image.fixture_effects().effects()[0].dispatch_at.is_none());
    assert!(
        image.fixture_effects().effects()[0]
            .process_observations
            .is_empty()
    );
}
