//! SOURCE_SIMULATION. Canonical journal transitions use synthetic observer inputs.
use super::*;
use crate::domain_runtime::tests::fixture;
use cortex_core::{
    binding::ObservationId,
    program::{OperationId, supervision::ProcessCompletion},
};
use cortex_storage::{
    history_store::{Access, StoreKey},
    program_store::{ProgramStore, ReadLimits},
};
use std::{fs, os::unix::fs::DirBuilderExt, path::PathBuf, time::Duration};

struct Fixture {
    root: PathBuf,
    store: ProgramStore,
    key: StoreKey,
    intent: DomainIntent,
    end: ManagedEndObservation,
    start: Instant,
    witness: NormalEndCpuWitness,
}

fn source(number: u64) -> SourceReference {
    SourceReference::observation_reference(
        ObservationId::new("independent-source-simulation", number).unwrap(),
        "simulated evidence",
        "1",
    )
    .unwrap()
}

impl Fixture {
    fn new(case: &str, terminal_micros: u64) -> Self {
        let (mut kernel, intent) = fixture::coordinator();
        let identity = fixture::identity(&intent);
        for (id, event) in [
            (
                "reserve",
                DomainEvent::Reserve {
                    intent: intent.clone(),
                    at: 1,
                },
            ),
            (
                "create",
                DomainEvent::CreateRequested {
                    domain: intent.id.clone(),
                    at: 2,
                },
            ),
            (
                "bind",
                DomainEvent::BoundObserved {
                    domain: intent.id.clone(),
                    observation: fixture::binding(&intent),
                    at: 3,
                },
            ),
            (
                "release",
                DomainEvent::ReleaseRequested {
                    domain: intent.id.clone(),
                    identity: identity.clone(),
                    at: 4,
                },
            ),
            (
                "report",
                DomainEvent::Report {
                    domain: intent.id.clone(),
                    identity: identity.clone(),
                    outcome: EffectOutcome::Succeeded,
                    at: 5,
                    basis: fixture::source("source-simulation-report"),
                },
            ),
        ] {
            fixture::event(&mut kernel, id, event);
        }

        let root =
            std::env::temp_dir().join(format!("cortex-normal-end-{}-{case}", std::process::id()));
        fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
        let key = StoreKey::new(&[42; 32]).unwrap();
        let store = ProgramStore::create_domain_fixture(
            &root.join("store"),
            &key,
            kernel.initial().clone(),
            ReadLimits::new(16_000_000, Duration::from_secs(5)).unwrap(),
        )
        .unwrap();
        for operation in kernel.operations() {
            store.admit(&key, operation.clone()).unwrap();
        }

        let start = Instant::now();
        Self {
            root,
            store,
            key,
            end: ManagedEndObservation {
                domain: intent.id.clone(),
                release: OperationId::new("release").unwrap(),
                binding_identity: identity.clone(),
                observed_at: start + Duration::from_millis(3),
                completion: ProcessCompletion::Exited {
                    code: Some(0),
                    signal: None,
                },
                transport_complete: true,
                readback: ManagedEndReadback::Observed {
                    populated: false,
                    cpu: CpuCounterObservation::LowerBoundMicros(1001),
                },
            },
            start,
            witness: NormalEndCpuWitness {
                baseline: TerminalCpuRead {
                    identity: identity.clone(),
                    at: start + Duration::from_millis(1),
                    usage_micros: 0,
                },
                terminal: TerminalCpuRead {
                    identity,
                    at: start + Duration::from_millis(4),
                    usage_micros: terminal_micros,
                },
                lifetime: source(1),
                population_and_streams: source(2),
                terminal_fence: source(3),
                receipt: source(4),
            },
            intent,
        }
    }

    fn load(&self) -> CoordinationKernel {
        self.store.load(&self.key, Access::ReadOnly).unwrap()
    }

    fn check(
        &self,
        kernel: &CoordinationKernel,
        operation: &Operation,
        witness: Option<&NormalEndCpuWitness>,
    ) -> Result<u64, NormalEndClosureError> {
        check_normal_end_cpu_closure(
            kernel,
            NormalEndContext {
                intent: &self.intent,
                observation: &self.end,
                start: self.start,
                end: self.start + Duration::from_millis(10),
            },
            witness,
            operation,
        )
    }

    fn close(&self, kernel: &CoordinationKernel, usage: Option<u64>) -> Operation {
        let mut receipt = fixture::closure(&self.intent, usage);
        receipt.evidence = self.witness.receipt.clone();
        fixture::domain_operation(
            kernel,
            "close",
            Command::Domain(Box::new(DomainEvent::Closed { receipt, at: 6 })),
        )
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn normal_end_keeps_cpu_debt_without_exact_independent_inputs() {
    use NormalEndClosureError::*;
    let mut f = Fixture::new("refusals", 2001);
    let kernel = f.load();
    let op = f.close(&kernel, Some(3));
    assert_eq!(f.check(&kernel, &op, None), Err(WitnessRequired));
    assert_eq!(
        f.check(&kernel, &f.close(&kernel, Some(2)), Some(&f.witness)),
        Err(CpuDebit)
    );
    f.witness.terminal.usage_micros = 1000;
    assert_eq!(
        f.check(&kernel, &op, Some(&f.witness)),
        Err(CounterContinuity)
    );
    f.witness.terminal.usage_micros = 2001;
    f.witness.baseline.usage_micros = 1;
    assert_eq!(
        f.check(&kernel, &op, Some(&f.witness)),
        Err(CounterContinuity)
    );
    f.witness.baseline.usage_micros = 0;
    f.witness.terminal.at = f.start + Duration::from_millis(10);
    assert_eq!(f.check(&kernel, &op, Some(&f.witness)), Err(WitnessTiming));
    f.witness.terminal.at = f.start + Duration::from_millis(4);
    f.witness.terminal.identity.cgroup_inode += 1;
    assert_eq!(f.check(&kernel, &op, Some(&f.witness)), Err(Correspondence));
    f.witness.terminal.identity = f.end.binding_identity.clone();
    f.witness.terminal_fence = fixture::source("unverified fence assertion");
    assert_eq!(
        f.check(&kernel, &op, Some(&f.witness)),
        Err(WitnessRequired)
    );
    f.witness.terminal_fence = source(3);

    f.end.transport_complete = false;
    assert_eq!(f.check(&kernel, &op, Some(&f.witness)), Err(NotNormalEnd));
    f.end.transport_complete = true;
    f.end.readback = ManagedEndReadback::Observed {
        populated: true,
        cpu: CpuCounterObservation::LowerBoundMicros(2001),
    };
    assert_eq!(f.check(&kernel, &op, Some(&f.witness)), Err(NotNormalEnd));
    f.end.readback = ManagedEndReadback::Observed {
        populated: false,
        cpu: CpuCounterObservation::Contradictory,
    };
    assert_eq!(
        f.check(&kernel, &op, Some(&f.witness)),
        Err(CounterUnavailable)
    );
    f.end.release = OperationId::new("report").unwrap();
    assert_eq!(f.check(&kernel, &op, Some(&f.witness)), Err(Correspondence));

    let unchanged = f.load();
    assert_eq!(unchanged.revision(), kernel.revision());
    let record = unchanged.domains().unwrap().domain(&f.intent.id).unwrap();
    assert!(record.unresolved_consumption(Consumption::CpuExecutionMillis));
    assert_eq!(
        record.outcome,
        HistoricalOutcome::Reported(EffectOutcome::Succeeded)
    );
}

#[test]
fn normal_end_witness_screens_existing_closed_and_settle_without_changing_journal() {
    let f = Fixture::new("closed", 2001);
    let kernel = f.load();
    let op = f.close(&kernel, Some(3));
    assert_eq!(f.check(&kernel, &op, Some(&f.witness)), Ok(3));
    assert_eq!(f.load().revision(), kernel.revision());
    let mut stale = op.clone();
    stale.expected -= 1;
    assert!(matches!(
        f.check(&kernel, &stale, Some(&f.witness)),
        Err(NormalEndClosureError::Canonical(_))
    ));
    let first = f.store.admit(&f.key, op.clone()).unwrap();
    let duplicate = f.store.admit(&f.key, op).unwrap();
    assert!(duplicate.duplicate);
    assert_eq!(duplicate.ordinal, first.ordinal);
    assert_eq!(
        f.load()
            .domains()
            .unwrap()
            .domain(&f.intent.id)
            .unwrap()
            .measured_consumption(Consumption::CpuExecutionMillis),
        Some(3)
    );

    let f = Fixture::new("settle", u64::MAX);
    let op = f.close(&f.load(), None);
    f.store.admit(&f.key, op).unwrap();
    let kernel = f.load();
    let mut receipt = fixture::closure(&f.intent, Some(18_446_744_073_709_552));
    receipt.evidence = f.witness.receipt.clone();
    receipt.released.clear();
    let op = fixture::domain_operation(
        &kernel,
        "settle",
        Command::Domain(Box::new(DomainEvent::Settle { receipt, at: 7 })),
    );
    assert_eq!(
        f.check(&kernel, &op, Some(&f.witness)),
        Ok(18_446_744_073_709_552)
    );
    assert_eq!(f.load().revision(), kernel.revision());
    f.store.admit(&f.key, op).unwrap();
    let current = f.load();
    let record = current.domains().unwrap().domain(&f.intent.id).unwrap();
    assert_eq!(record.settlements.len(), 1);
    assert_eq!(
        record.measured_consumption(Consumption::CpuExecutionMillis),
        Some(18_446_744_073_709_552)
    );
}
