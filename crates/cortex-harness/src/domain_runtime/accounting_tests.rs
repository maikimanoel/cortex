//! SOURCE_SIMULATION. Observations and independent closure assertions below are
//! synthetic. Every durable decision is made by the canonical store.
use super::*;
use crate::native_execution::accounting::{
    CpuClosureCheckError, check_cpu_closure_against_sample, parse_cpu_usage,
};
use cortex_evidence::program::KernelError;
use cortex_storage::{history_store::Completion, program_store::ProgramStoreError};

const CPU: Consumption = Consumption::CpuExecutionMillis;

fn input(k: &CoordinationKernel, id: &str, event: DomainEvent) -> Operation {
    fixture::domain_operation(k, id, Command::Domain(Box::new(event)))
}

fn load(store: &ProgramStore) -> CoordinationKernel {
    store.load(&key(), Access::ReadOnly).unwrap()
}

fn tick(k: &CoordinationKernel) -> u64 {
    k.domains().unwrap().last_tick() + 1
}

fn sample(store: &ProgramStore, i: &DomainIntent, label: &str, micros: Option<u64>) -> u64 {
    let k = load(store);
    let at = tick(&k);
    let cancel = input(
        &k,
        &format!("sim-cancel-{label}"),
        DomainEvent::Cancel {
            domain: i.id.clone(),
            at,
            basis: fixture::source("simulation-cancel"),
        },
    );
    let grant = cancel.grant.clone();
    store.admit(&key(), cancel).unwrap();
    let root = Temp::new();
    let mut fake = LifecycleManager::new(&root, i);
    fake.synthetic_empty_tree = true;
    fake.synthetic_cpu_micros = micros;
    let ids = DomainOperations {
        domain: i.id.clone(),
        create: OperationId::new("unused-create").unwrap(),
        bind: OperationId::new("unused-bind").unwrap(),
        release: OperationId::new("unused-release").unwrap(),
        report: OperationId::new("unused-report").unwrap(),
        grant,
        at,
        basis: fixture::source("synthetic-counter-sample"),
    };
    let start = Instant::now();
    let id = OperationId::new(&format!("sim-sample-{label}")).unwrap();
    let op = capture_closure_tree(
        store,
        &key(),
        &mut fake,
        &ids,
        id,
        start,
        start + Duration::from_secs(1),
    )
    .unwrap();
    let Command::Domain(tree) = &op.command else {
        panic!("wrong command");
    };
    let DomainEvent::ClosureTreeObserved { at: captured, .. } = *tree.as_ref() else {
        panic!("fixture did not use canonical tree observation");
    };
    store.admit(&key(), op).unwrap();
    assert_eq!((fake.inner.created, fake.inner.stopped), (0, 0));
    captured
}

fn check_debt(store: &ProgramStore, f: &DomainIntent) {
    let k = load(store);
    let reserve = input(
        &k,
        "reserve-dependent",
        DomainEvent::Reserve {
            intent: f.clone(),
            at: tick(&k),
        },
    );
    assert!(matches!(
        k.preview_admission(&reserve),
        Err(KernelError::Domain(DomainError::Debt(CPU)))
    ));
    assert!(k.domains().unwrap().domain(&f.id).is_none());
    let decisions =
        admit_domain_reservations(store, &key(), vec![reserve], NonZeroUsize::MIN).unwrap();
    let [ReservationDecision::Held(refused)] = decisions.as_slice() else {
        panic!("dependent reservation did not hold: {decisions:?}");
    };
    assert_eq!(refused.completion, Completion::Rejected);
    assert!(matches!(
        refused.error,
        ProgramStoreError::Domain(KernelError::Domain(DomainError::Debt(CPU)))
    ));
    assert_eq!(load(store).revision(), k.revision());
}

fn close_assertion(intent: &DomainIntent, usage: Option<u64>) -> ClosureReceipt {
    // Existing independently asserted closure profile, applied only to a fresh
    // test journal. It deliberately does not derive coverage from the sample.
    let mut receipt = fixture::closure(intent, usage);
    receipt.evidence = fixture::source("independent-simulated-closure");
    receipt
}

pub(super) fn exercise_settlement(
    store: &ProgramStore,
    b: &DomainIntent,
    e: &DomainIntent,
    f: &DomainIntent,
) {
    let b_at = sample(
        store,
        b,
        "B",
        Some(parse_cpu_usage("usage_usec 1001\n").unwrap()),
    );
    let e_at = sample(
        store,
        e,
        "E",
        Some(parse_cpu_usage("usage_usec 9201\n").unwrap()),
    );
    check_debt(store, f);
    let k = load(store);
    let domains = k.domains().unwrap();
    let b_record = domains.domain(&b.id).unwrap();
    let e_record = domains.domain(&e.id).unwrap();
    assert_eq!(
        check_cpu_closure_against_sample(b_record, b_at, &close_assertion(b, Some(1))),
        Err(CpuClosureCheckError::Undercount)
    );
    assert_eq!(
        check_cpu_closure_against_sample(b_record, b_at, &close_assertion(b, Some(2))),
        Ok(2)
    );
    assert_eq!(
        check_cpu_closure_against_sample(e_record, e_at, &close_assertion(e, Some(10))),
        Ok(10)
    );

    // Capacity disposition can be observed while final CPU remains unresolved.
    let e_close = input(
        &k,
        "close-E",
        DomainEvent::Closed {
            receipt: close_assertion(e, None),
            at: tick(&k),
        },
    );
    store.admit(&key(), e_close).unwrap();
    let k = load(store);
    let b_close = input(
        &k,
        "close-B",
        DomainEvent::Closed {
            receipt: close_assertion(b, Some(2)),
            at: tick(&k),
        },
    );
    let accepted = store.admit(&key(), b_close.clone()).unwrap();
    assert!(!accepted.duplicate);
    let repeated = store.admit(&key(), b_close.clone()).unwrap();
    assert!(repeated.duplicate);
    assert_eq!(accepted.ordinal, repeated.ordinal);
    let mut conflict = b_close;
    let Command::Domain(ref mut event) = conflict.command else {
        panic!()
    };
    let DomainEvent::Closed { receipt, .. } = event.as_mut() else {
        panic!()
    };
    receipt.consumption[0].measured = Some(9);
    assert!(matches!(
        store.admit(&key(), conflict).unwrap_err().error,
        ProgramStoreError::Domain(KernelError::Core(
            cortex_core::program::ProgramError::IdentityConflict
        ))
    ));
    check_debt(store, f);

    let k = load(store);
    let mut debit = close_assertion(e, Some(10));
    debit.released.clear();
    let mut excessive = debit.clone();
    excessive.consumption[0].measured = Some(181);
    let mut over_budget = k.clone();
    let event = DomainEvent::Settle {
        receipt: excessive,
        at: tick(&k),
    };
    over_budget
        .admit(input(&k, "over-budget-observation", event))
        .unwrap();
    assert_eq!(
        over_budget.domains().unwrap().consumed_or_reserved(CPU),
        Ok(183)
    );
    let denied = over_budget
        .admit(input(
            &over_budget,
            "over-budget-reserve",
            DomainEvent::Reserve {
                intent: f.clone(),
                at: tick(&over_budget),
            },
        ))
        .unwrap_err();
    assert_eq!(denied, KernelError::Domain(DomainError::Budget(CPU)));

    let settle = input(
        &k,
        "settle-E",
        DomainEvent::Settle {
            receipt: debit,
            at: tick(&k),
        },
    );
    let barrier = std::sync::Barrier::new(2);
    let (one, two) = thread::scope(|scope| {
        let submit = || {
            barrier.wait();
            store.admit(&key(), settle.clone()).unwrap()
        };
        let one = scope.spawn(submit);
        let two = scope.spawn(submit);
        (one.join().unwrap(), two.join().unwrap())
    });
    assert_ne!(one.duplicate, two.duplicate);
    assert_eq!(one.ordinal, two.ordinal);
    let k = load(store);
    let domain = k.domains().unwrap().domain(&e.id).unwrap();
    assert_eq!(domain.settlements.len(), 1);
    assert_eq!(domain.measured_consumption(CPU), Some(10));
    assert_eq!(k.domains().unwrap().consumed_or_reserved(CPU), Ok(12));

    // A distinct stale operation cannot use the same previous revision to
    // admit another copy of F. Neither operation dispatches a process.
    let valid = input(
        &k,
        "fresh-F",
        DomainEvent::Reserve {
            intent: f.clone(),
            at: tick(&k),
        },
    );
    let preview = k.preview_admission(&valid).unwrap();
    assert_eq!(preview.observed_revision(), k.revision());
    assert_eq!(preview.resulting_ordinal(), k.revision() + 1);
    assert!(!preview.is_duplicate());
    assert!(k.domains().unwrap().domain(&f.id).is_none());
    assert_eq!(load(store).revision(), k.revision());

    let mut stale = valid.clone();
    stale.id = OperationId::new("stale-F").unwrap();
    let barrier = std::sync::Barrier::new(2);
    let (one, two) = thread::scope(|scope| {
        let submit = |operation| {
            barrier.wait();
            admit_domain_reservations(store, &key(), vec![operation], NonZeroUsize::MIN)
                .unwrap()
                .pop()
                .unwrap()
        };
        let one = scope.spawn(move || submit(valid));
        let two = scope.spawn(move || submit(stale));
        (one.join().unwrap(), two.join().unwrap())
    });
    let refusal = match (one, two) {
        (ReservationDecision::Reserved(receipt), ReservationDecision::Held(refused))
        | (ReservationDecision::Held(refused), ReservationDecision::Reserved(receipt)) => {
            assert!(!receipt.duplicate);
            refused
        }
        _ => panic!("exactly one new F reservation must commit"),
    };
    assert_eq!(refusal.completion, Completion::Rejected);
    assert!(matches!(
        refusal.error,
        ProgramStoreError::Domain(KernelError::Core(cortex_core::program::ProgramError::Stale))
    ));
    assert_eq!(load(store).revision(), k.revision() + 1);
    assert_eq!(
        load(store).domains().unwrap().consumed_or_reserved(CPU),
        Ok(42)
    );
}

#[test]
fn cpu_corroboration_refuses_incomplete_stale_and_cross_incarnation_claims() {
    let root = Temp::new();
    let (store, i, ids) = stored(&root);
    let component = Temp::new();
    let mut manager = LifecycleManager::new(&component, &i);
    let run = execute_lifecycle(&store, &mut manager, &ids);
    manager.inner.finish();
    assert!(run.report.is_some());
    let at = sample(&store, &i, "single", Some(1001));
    let kernel = load(&store);
    let exact = kernel.domains().unwrap().domain(&i.id).unwrap();
    let receipt = close_assertion(&i, Some(2));
    assert_eq!(check_cpu_closure_against_sample(exact, at, &receipt), Ok(2));

    for mutate in [0, 1, 2, 3, 4, 5] {
        let mut wrong = receipt.clone();
        match mutate {
            0 => wrong.identity.invocation.push('X'),
            1 => wrong.identity.boot.push('X'),
            2 => wrong.identity.cgroup_inode += 1,
            3 => wrong.configuration = fixture::source("different-executable"),
            4 => wrong.scope.generation = fixture::rev(2),
            5 => wrong.domain = DomainId::new("different-domain").unwrap(),
            _ => unreachable!(),
        }
        assert_eq!(
            check_cpu_closure_against_sample(exact, at, &wrong),
            Err(CpuClosureCheckError::Correspondence)
        );
    }
    let mut incomplete = receipt.clone();
    incomplete.snapshot_safe = false;
    assert_eq!(
        check_cpu_closure_against_sample(exact, at, &incomplete),
        Err(CpuClosureCheckError::Incomplete)
    );
    let mut incomplete = receipt.clone();
    incomplete.evidence = exact.closure_tree_observations[0].sample.basis.clone();
    assert_eq!(
        check_cpu_closure_against_sample(exact, at, &incomplete),
        Err(CpuClosureCheckError::Incomplete)
    );
    assert_eq!(
        check_cpu_closure_against_sample(exact, at + 1, &receipt),
        Err(CpuClosureCheckError::StaleSample)
    );
    let mut populated = exact.clone();
    let last = populated.closure_tree_observations.last_mut().unwrap();
    last.nodes[0].populated = true;
    last.sample.populated = true;
    assert_eq!(
        check_cpu_closure_against_sample(&populated, at, &receipt),
        Err(CpuClosureCheckError::Incomplete)
    );
    let mut missing = exact.clone();
    missing.closure_tree_observations[0].sample.cpu_micros = None;
    assert_eq!(
        check_cpu_closure_against_sample(&missing, at, &receipt),
        Err(CpuClosureCheckError::MissingSample)
    );
    let mut reset = exact.clone();
    let mut later = reset.closure_tree_observations[0].clone();
    later.sample.observed_at += 1;
    later.sample.cpu_micros = Some(0);
    reset.closure_tree_observations.push(later);
    assert_eq!(
        check_cpu_closure_against_sample(&reset, at + 1, &receipt),
        Err(CpuClosureCheckError::StaleSample)
    );
    assert!(exact.unresolved_consumption(CPU));
    assert_eq!(load(&store).revision(), kernel.revision());
}

#[test]
fn mixed_closure_history_cannot_hide_a_counter_decrease() {
    let root = Temp::new();
    let (store, intent, ids) = stored(&root);
    let component = Temp::new();
    let mut manager = LifecycleManager::new(&component, &intent);
    let run = execute_lifecycle(&store, &mut manager, &ids);
    manager.inner.finish();
    assert!(run.report.is_some());
    sample(&store, &intent, "mixed-history", Some(9_000));
    let kernel = load(&store);
    let domain = kernel.domains().unwrap().domain(&intent.id).unwrap();
    let mut tree = domain.closure_tree_observations[0].clone();
    let mut direct = tree.sample.clone();
    direct.observed_at = tick(&kernel);
    direct.cpu_micros = Some(1_000);
    let at = direct.observed_at;
    store
        .admit(
            &key(),
            input(
                &kernel,
                "mixed-direct-sample",
                DomainEvent::ClosureObserved {
                    domain: intent.id.clone(),
                    observation: direct,
                    at,
                },
            ),
        )
        .unwrap();
    let kernel = load(&store);
    tree.sample.observed_at = tick(&kernel);
    tree.sample.cpu_micros = Some(10_000);
    let at = tree.sample.observed_at;
    store
        .admit(
            &key(),
            input(
                &kernel,
                "mixed-tree-sample",
                DomainEvent::ClosureTreeObserved {
                    domain: intent.id.clone(),
                    observation: tree,
                    at,
                },
            ),
        )
        .unwrap();
    let kernel = load(&store);
    let domain = kernel.domains().unwrap().domain(&intent.id).unwrap();
    assert_eq!(
        check_cpu_closure_against_sample(domain, at, &close_assertion(&intent, Some(10))),
        Err(CpuClosureCheckError::StaleSample)
    );
    assert!(domain.unresolved_consumption(CPU));
}

#[test]
fn cpu_sampling_rounds_up_even_at_the_integer_limit() {
    let root = Temp::new();
    let (store, i, ids) = stored(&root);
    let component = Temp::new();
    let mut manager = LifecycleManager::new(&component, &i);
    let run = execute_lifecycle(&store, &mut manager, &ids);
    manager.inner.finish();
    assert!(run.report.is_some());
    let at = sample(&store, &i, "range", Some(0));
    let k = load(&store);
    let mut record = k.domains().unwrap().domain(&i.id).unwrap().clone();
    for (microseconds, expected) in [
        (0, 0),
        (1, 1),
        (999, 1),
        (1000, 1),
        (1001, 2),
        (u64::MAX, 18_446_744_073_709_552),
    ] {
        record.closure_tree_observations[0].sample.cpu_micros = Some(microseconds);
        assert_eq!(
            check_cpu_closure_against_sample(&record, at, &close_assertion(&i, Some(expected))),
            Ok(expected)
        );
        if expected > 0 {
            assert_eq!(
                check_cpu_closure_against_sample(
                    &record,
                    at,
                    &close_assertion(&i, Some(expected - 1))
                ),
                Err(CpuClosureCheckError::Undercount)
            );
        }
    }
}

#[test]
fn partial_accounting_capture_requires_cancellation_and_the_original_open_window() {
    let root = Temp::new();
    let (store, intent, ids) = stored(&root);
    let component = Temp::new();
    let mut manager = LifecycleManager::new(&component, &intent);
    let result = execute_lifecycle(&store, &mut manager, &ids);
    manager.inner.finish();
    assert!(result.report.is_some());
    let revision = load(&store).revision();
    let observation = OperationId::new("simulation-unpublished").unwrap();
    let start = Instant::now();
    let observed = manager.inner.observed;
    let refused = capture_closure_tree(
        &store,
        &key(),
        &mut manager,
        &ids,
        observation.clone(),
        start,
        start,
    )
    .err()
    .unwrap();
    assert_eq!(refused.cause, DomainCause::Deadline);
    let refused = capture_closure_tree(
        &store,
        &key(),
        &mut manager,
        &ids,
        observation,
        start,
        start + Duration::from_secs(1),
    )
    .err()
    .unwrap();
    assert_eq!(refused.cause, DomainCause::Contract);
    assert_eq!(manager.inner.observed, observed);
    assert_eq!(load(&store).revision(), revision);
}

#[test]
fn successful_report_and_missing_counter_do_not_settle_after_reopen() {
    use cortex_core::program::operational::EffectOutcome;
    let root = Temp::new();
    let (store, intent, ids) = stored(&root);
    let component = Temp::new();
    let mut manager = LifecycleManager::new(&component, &intent);
    let run = execute_lifecycle(&store, &mut manager, &ids);
    manager.inner.finish();
    assert!(run.report.is_some() && run.diagnostic.is_none());
    assert_eq!((manager.inner.created, manager.completions), (1, 1));

    let before = load(&store);
    let report = before.domains().unwrap().domain(&intent.id).unwrap();
    assert_eq!(
        report.outcome,
        HistoricalOutcome::Reported(EffectOutcome::Succeeded)
    );
    assert!(!report.cancelled);
    assert!(report.holds(Resource::DiskBytes) && report.unresolved_consumption(CPU));
    let at = sample(&store, &intent, "unavailable-counter", None);
    let store_id = store.id();
    let limits = store.read_limits();
    drop(store);

    let mut zero = close_assertion(&intent, Some(0));
    let durable_revision;
    {
        let reopened =
            ProgramStore::open(&root.0.join("store"), &key(), Access::ReadOnly, limits).unwrap();
        assert!(reopened.id() == store_id);
        let k = load(&reopened);
        durable_revision = k.revision();
        assert_eq!(durable_revision, before.revision() + 2);
        let record = k.domains().unwrap().domain(&intent.id).unwrap();
        assert_eq!(record.closure_tree_observations.len(), 1);
        let observation = &record.closure_tree_observations[0].sample;
        assert_eq!(observation.observed_at, at);
        assert!(!observation.populated);
        assert_eq!(observation.cpu_micros, None);
        assert!(record.cancelled && record.closure.is_none());
        assert_eq!(
            record.outcome,
            HistoricalOutcome::Reported(EffectOutcome::Succeeded)
        );
        assert_eq!(
            check_cpu_closure_against_sample(record, at, &zero),
            Err(CpuClosureCheckError::MissingSample)
        );
    }

    let store = ProgramStore::open(&root.0.join("store"), &key(), Access::Recover, limits).unwrap();
    let k = load(&store);
    let attempt_settlement = input(
        &k,
        "premature-zero-settlement",
        DomainEvent::Settle {
            receipt: zero.clone(),
            at: tick(&k),
        },
    );
    zero.whole_domain = false;
    let attempt_closure = input(
        &k,
        "premature-closure-from-empty-sample",
        DomainEvent::Closed {
            receipt: zero,
            at: tick(&k),
        },
    );
    for operation in [attempt_settlement, attempt_closure] {
        let refused = store.admit(&key(), operation).unwrap_err();
        assert_eq!(refused.completion, Completion::Rejected);
        assert!(matches!(
            refused.error,
            ProgramStoreError::Domain(KernelError::Domain(DomainError::Correspondence))
        ));
        assert_eq!(load(&store).revision(), durable_revision);
    }

    let after = load(&store);
    let record = after.domains().unwrap().domain(&intent.id).unwrap();
    assert_eq!(
        record.outcome,
        HistoricalOutcome::Reported(EffectOutcome::Succeeded)
    );
    assert!(record.closure.is_none() && record.holds(Resource::DiskBytes));
    assert!(record.holds(Resource::WorkerSlots) && record.unresolved_consumption(CPU));
    assert_eq!(record.measured_consumption(CPU), None);
}
