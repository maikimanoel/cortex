//! Controlled manager and fixed Rust bootstrap, never a real manager activation.
use super::*;
use crate::supervisor::tests::fixture as program_fixture;
#[path = "../../../cortex-evidence/tests/support/domain.rs"]
pub(crate) mod fixture;
use cortex_storage::program_store::ReadLimits;
use std::{
    fs,
    os::unix::{fs::PermissionsExt, net::UnixStream},
    path::PathBuf,
    sync::{Arc, atomic::AtomicU64},
};

struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        static N: AtomicU64 = AtomicU64::new(0);
        let p = std::env::temp_dir().join(format!(
            "cortex-domain-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&p).unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(0o700)).unwrap();
        Self(p)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn key() -> StoreKey {
    StoreKey::new(&[0x34; 32]).unwrap()
}
fn stored(root: &Temp) -> (ProgramStore, DomainIntent, DomainOperations) {
    let mut kernel = CoordinationKernel::new_domain_fixture(fixture::definition()).unwrap();
    let mut pool = fixture::pool();
    pool.identity.boot = fs::read_to_string("/proc/sys/kernel/random/boot_id")
        .unwrap()
        .trim()
        .into();
    fixture::event(
        &mut kernel,
        "pool",
        DomainEvent::ConfigurePool(pool.clone()),
    );
    let mut intent = fixture::setup(&mut kernel, "A");
    intent.pool = pool.identity;
    fixture::event(
        &mut kernel,
        "reserve",
        DomainEvent::Reserve {
            intent: intent.clone(),
            at: 1,
        },
    );
    let store = ProgramStore::create_domain_fixture(
        &root.0.join("store"),
        &key(),
        kernel.initial().clone(),
        ReadLimits::new(16_000_000, Duration::from_secs(5)).unwrap(),
    )
    .unwrap();
    for op in kernel.operations() {
        store.admit(&key(), op.clone()).unwrap();
    }
    let grant = fixture::domain_operation(
        &kernel,
        "unused",
        Command::Domain(Box::new(DomainEvent::Unknown {
            domain: intent.id.clone(),
            at: 2,
            basis: fixture::source("unused"),
        })),
    )
    .grant;
    let ids = DomainOperations {
        domain: intent.id.clone(),
        create: OperationId::new("native-create").unwrap(),
        bind: OperationId::new("native-bind").unwrap(),
        release: OperationId::new("native-release").unwrap(),
        report: OperationId::new("native-report").unwrap(),
        grant,
        basis: fixture::source("controlled-manager"),
        at: 2,
    };
    (store, intent, ids)
}
struct FakeManager {
    endpoint: PathBuf,
    created: usize,
    stopped: usize,
    observed: usize,
    change_on_observation: Option<usize>,
    lost: bool,
    replace: bool,
    cancel: Option<Arc<AtomicBool>>,
    worker: Option<thread::JoinHandle<io::Result<()>>>,
}
impl FakeManager {
    fn new(root: &Temp) -> Self {
        Self {
            endpoint: root.0.join("control"),
            created: 0,
            stopped: 0,
            observed: 0,
            change_on_observation: None,
            lost: false,
            replace: false,
            cancel: None,
            worker: None,
        }
    }
    fn finish(&mut self) {
        if let Some(w) = self.worker.take() {
            let _ = w.join().unwrap();
        }
    }
}
impl DomainManager for FakeManager {
    fn corresponds(&self, _: &DomainIntent) -> Result<(), DomainDiagnostic> {
        Ok(())
    }
    fn create(&mut self, intent: &DomainIntent, _: Instant) -> Result<(), DomainDiagnostic> {
        self.created += 1;
        let endpoint = self.endpoint.clone();
        let request = intent.request.as_str().to_string();
        self.worker = Some(thread::spawn(move || {
            crate::domain_bootstrap::run_nonce_bootstrap(
                UnixStream::connect(endpoint)?,
                &request,
                Duration::from_secs(1),
            )
        }));
        if self.lost {
            Err(issue(
                DomainPhaseDiagnostic::Creation,
                DomainCause::ManagerRefusal,
            ))
        } else {
            Ok(())
        }
    }
    fn observe(
        &mut self,
        intent: &DomainIntent,
        _: Instant,
    ) -> Result<ManagedObservation, DomainDiagnostic> {
        self.observed += 1;
        if let Some(c) = &self.cancel {
            c.store(true, Ordering::SeqCst)
        }
        let mut identity = fixture::identity(intent);
        if (self.replace && self.observed > 1) || self.change_on_observation == Some(self.observed)
        {
            identity.invocation = "replacement".into();
        }
        Ok(ManagedObservation {
            identity,
            main_pid: std::process::id(),
            start_ticks: crate::domain_bootstrap::ticks(std::process::id()).unwrap(),
            populated: true,
            direct_members: vec![DomainProcessObservation {
                pid: std::process::id(),
                start_ticks: crate::domain_bootstrap::ticks(std::process::id()).unwrap(),
            }],
            controls: fixture::binding(intent).controls,
            cpu_micros: None,
            memory_peak: None,
            restart_prevented: true,
        })
    }
    fn stop(
        &mut self,
        intent: &DomainIntent,
        identity: &DomainIdentity,
        end: Instant,
    ) -> Result<(), DomainDiagnostic> {
        if self.observe(intent, end)?.identity != *identity {
            return Err(issue(
                DomainPhaseDiagnostic::Closure,
                DomainCause::Correspondence,
            ));
        }
        self.stopped += 1;
        Ok(())
    }
}
fn run(
    store: &ProgramStore,
    m: &mut FakeManager,
    ids: &DomainOperations,
    cancel: &AtomicBool,
) -> DomainRun {
    let endpoint = m.endpoint.clone();
    execute(
        store,
        &key(),
        m,
        ExecutionRequest {
            ids,
            nonce: "public-nonce",
            cancel,
            start: Instant::now(),
            end: Instant::now() + Duration::from_secs(1),
        },
        || {
            UnixListener::bind(endpoint)
                .map_err(|e| issue(DomainPhaseDiagnostic::Creation, DomainCause::Io(e.kind())))
        },
    )
}

#[test]
fn controlled_manager_bootstrap_journal_chain_produces_nonce_without_settlement() {
    let root = Temp::new();
    let (store, intent, ids) = stored(&root);
    let mut manager = FakeManager::new(&root);
    let result = run(&store, &mut manager, &ids, &AtomicBool::new(false));
    manager.finish();
    assert!(result.diagnostic.is_none(), "{:?}", result.diagnostic);
    assert_eq!(result.nonce.as_deref(), Some("public-nonce"));
    assert!(
        result.creation.is_some()
            && result.binding.is_some()
            && result.release.is_some()
            && result.report.is_some()
    );
    let k = store.load(&key(), Access::ReadOnly).unwrap();
    let r = k.domains().unwrap().domain(&intent.id).unwrap();
    assert_eq!(r.phase, DomainPhase::ReleaseUncertain);
    assert!(r.holds(cortex_core::program::scheduling::Resource::Workers));
    assert_eq!(manager.created, 1);
}
#[test]
fn lost_create_reply_reconstructs_uncertainty_and_cannot_create_twice() {
    let root = Temp::new();
    let (store, intent, ids) = stored(&root);
    let mut manager = FakeManager::new(&root);
    manager.lost = true;
    let first = run(&store, &mut manager, &ids, &AtomicBool::new(false));
    manager.finish();
    assert!(first.diagnostic.is_some());
    let reopened = ProgramStore::open(
        &root.0.join("store"),
        &key(),
        Access::ReadOnly,
        ReadLimits::new(16_000_000, Duration::from_secs(5)).unwrap(),
    )
    .unwrap();
    let next = run(&reopened, &mut manager, &ids, &AtomicBool::new(false));
    assert!(next.recovery_only && next.nonce.is_none());
    assert_eq!(manager.created, 1);
    let k = reopened.load(&key(), Access::ReadOnly).unwrap();
    let r = k.domains().unwrap().domain(&intent.id).unwrap();
    assert_eq!(r.phase, DomainPhase::CreateUncertain);
    assert_eq!(r.outcome, HistoricalOutcome::Unknown);
    assert!(r.identity.is_none());
}
#[test]
fn cancel_during_identity_acquisition_prevents_binding_and_release() {
    let root = Temp::new();
    let (store, _, ids) = stored(&root);
    let mut manager = FakeManager::new(&root);
    let cancel = Arc::new(AtomicBool::new(false));
    manager.cancel = Some(cancel.clone());
    let result = run(&store, &mut manager, &ids, &cancel);
    manager.finish();
    assert_eq!(result.diagnostic.unwrap().cause, DomainCause::Authority);
    assert!(result.binding.is_none() && result.release.is_none() && result.nonce.is_none());
}
#[test]
fn reused_manager_identity_after_publication_blocks_payload_release() {
    let root = Temp::new();
    let (store, _, ids) = stored(&root);
    let mut manager = FakeManager::new(&root);
    manager.replace = true;
    let result = run(&store, &mut manager, &ids, &AtomicBool::new(false));
    manager.finish();
    assert_eq!(
        result.diagnostic.unwrap().cause,
        DomainCause::Correspondence
    );
    assert!(result.binding.is_some() && result.release.is_none() && result.nonce.is_none());
}
#[test]
fn pre_dispatch_cancel_never_calls_manager_or_creates_endpoint() {
    let root = Temp::new();
    let (store, _, ids) = stored(&root);
    let mut manager = FakeManager::new(&root);
    let result = run(&store, &mut manager, &ids, &AtomicBool::new(true));
    assert!(result.diagnostic.is_some());
    assert_eq!(manager.created, 0);
    assert!(!manager.endpoint.exists());
}

fn controlled_recovery_continuation(measured: Option<u64>) {
    use cortex_core::program::scheduling::Resource;
    let root = Temp::new();
    let (store, original, ids) = stored(&root);
    let mut first = FakeManager::new(&root);
    first.lost = true;
    let result = run(&store, &mut first, &ids, &AtomicBool::new(false));
    first.finish(); // Controlled fixture thread observed joined, not a host fence.
    assert!(result.diagnostic.is_some());
    let mut kernel = store.load(&key(), Access::ReadOnly).unwrap();
    let prior = kernel.revision();
    let recovery_tick = kernel.domains().unwrap().last_tick() + 1;
    let closure = fixture::closure(&original, measured);
    let close = fixture::domain_operation(
        &kernel,
        "independent-controlled-close",
        Command::Domain(Box::new(DomainEvent::RecoveredClosed {
            receipt: closure,
            at: recovery_tick,
        })),
    );
    store.admit(&key(), close.clone()).unwrap();
    kernel.admit(close).unwrap();
    let closed = kernel.domains().unwrap().domain(&original.id).unwrap();
    assert_eq!(closed.outcome, HistoricalOutcome::Unknown);
    assert!(!closed.holds(Resource::Workers));
    assert!(closed.holds(Resource::DiskBytes));
    let mut next = fixture::add_member(&mut kernel, "independent-next-program");
    next.pool = original.pool.clone();
    for operation in &kernel.operations()[prior as usize + 1..] {
        store.admit(&key(), operation.clone()).unwrap();
    }
    let reserve = fixture::domain_operation(
        &kernel,
        "next-reservation",
        Command::Domain(Box::new(DomainEvent::Reserve {
            intent: next.clone(),
            at: recovery_tick + 1,
        })),
    );
    let admitted = store.admit(&key(), reserve);
    if measured.is_none() {
        assert!(admitted.is_err()); // Free occupancy does not erase cumulative debt.
        assert_eq!(first.created, 1);
        return;
    }
    admitted.unwrap();
    let second_root = Temp::new();
    let mut second = FakeManager::new(&second_root);
    let mut next_ids = ids;
    next_ids.domain = next.id;
    next_ids.create = OperationId::new("next-create").unwrap();
    next_ids.bind = OperationId::new("next-bind").unwrap();
    next_ids.release = OperationId::new("next-release").unwrap();
    next_ids.report = OperationId::new("next-report").unwrap();
    next_ids.at = recovery_tick + 2;
    let next_result = run(&store, &mut second, &next_ids, &AtomicBool::new(false));
    second.finish();
    assert!(
        next_result.diagnostic.is_none(),
        "{:?}",
        next_result.diagnostic
    );
    assert_eq!(next_result.nonce.as_deref(), Some("public-nonce"));
    assert_eq!(first.created, 1);
    assert_eq!(second.created, 1);
    let original = store
        .load(&key(), Access::ReadOnly)
        .unwrap()
        .domains()
        .unwrap()
        .domain(&original.id)
        .unwrap()
        .clone();
    assert_eq!(original.outcome, HistoricalOutcome::Unknown);
    assert!(original.holds(Resource::DiskBytes));
}

#[test]
fn controlled_lost_reply_closure_preserves_unknown_and_admits_independent_next_nonce() {
    controlled_recovery_continuation(Some(1));
}

#[test]
fn controlled_capacity_settlement_with_cpu_debt_cannot_admit_next_nonce() {
    controlled_recovery_continuation(None);
}

fn prepare_only(
    store: &ProgramStore,
    manager: &mut FakeManager,
    ids: &DomainOperations,
) -> DomainPreparation {
    let endpoint = manager.endpoint.clone();
    let start = Instant::now();
    prepare(
        store,
        &key(),
        manager,
        ExecutionRequest {
            ids,
            nonce: "public-nonce",
            cancel: &AtomicBool::new(false),
            start,
            end: start + Duration::from_secs(1),
        },
        || {
            UnixListener::bind(endpoint)
                .map_err(|e| issue(DomainPhaseDiagnostic::Creation, DomainCause::Io(e.kind())))
        },
    )
}
fn cancellation(store: &ProgramStore, ids: &DomainOperations) -> Operation {
    let k = store.load(&key(), Access::ReadOnly).unwrap();
    fixture::domain_operation(
        &k,
        "explicit-cancel",
        Command::Domain(Box::new(DomainEvent::Cancel {
            domain: ids.domain.clone(),
            at: k.domains().unwrap().last_tick(),
            basis: fixture::source("explicit-cancellation"),
        })),
    )
}

#[test]
fn preparation_retains_real_channel_without_release_until_explicit_consumption() {
    use std::io::Read;
    let root = Temp::new();
    let (store, intent, ids) = stored(&root);
    let mut manager = FakeManager::new(&root);
    let preparation = prepare_only(&store, &mut manager, &ids);
    assert!(
        preparation.run.diagnostic.is_none(),
        "{:?}",
        preparation.run.diagnostic
    );
    assert!(preparation.run.release.is_none() && preparation.run.nonce.is_none());
    let mut held = preparation.prepared.unwrap();
    held.channel.set_nonblocking(true).unwrap();
    assert_eq!(
        held.channel.read(&mut [0u8; 1]).unwrap_err().kind(),
        io::ErrorKind::WouldBlock
    );
    held.channel.set_nonblocking(false).unwrap();
    let restored = store.load(&key(), Access::ReadOnly).unwrap();
    let r = restored.domains().unwrap().domain(&intent.id).unwrap();
    assert_eq!(r.phase, DomainPhase::Bound);
    assert!(r.binding_observation.as_ref() == Some(held.observation()));
    assert!(!restored.operations().iter().any(|o| matches!(&o.command, Command::Domain(e) if matches!(e.as_ref(), DomainEvent::ReleaseRequested { .. }))));
    // Reconstruction cannot create a second channel or duplicate the effect.
    let duplicate = prepare_only(&store, &mut manager, &ids);
    assert!(duplicate.run.recovery_only && duplicate.prepared.is_none());
    assert_eq!(manager.created, 1);
    let result = release(&store, &key(), &mut manager, held, &AtomicBool::new(false));
    manager.finish();
    assert!(result.diagnostic.is_none(), "{:?}", result.diagnostic);
    assert_eq!(result.nonce.as_deref(), Some("public-nonce"));
    assert!(result.release.is_some());
}

#[test]
fn dropped_prepared_channel_preserves_bound_unknown_and_forbids_recreation() {
    let root = Temp::new();
    let (store, intent, ids) = stored(&root);
    let mut manager = FakeManager::new(&root);
    let preparation = prepare_only(&store, &mut manager, &ids);
    assert!(preparation.prepared.is_some());
    drop(preparation);
    manager.finish();
    let restored = store.load(&key(), Access::ReadOnly).unwrap();
    let r = restored.domains().unwrap().domain(&intent.id).unwrap();
    assert_eq!(r.phase, DomainPhase::Bound);
    assert_eq!(r.outcome, HistoricalOutcome::Unknown);
    assert!(!r.cancelled && r.closure.is_none());
    assert!(r.holds(cortex_core::program::scheduling::Resource::Workers));
    assert!(r.unresolved_consumption(Consumption::CpuExecutionMillis));
    let result = prepare_only(&store, &mut manager, &ids);
    assert!(result.run.recovery_only && result.prepared.is_none());
    assert_eq!(manager.created, 1);
}

#[test]
fn owned_prepared_cancel_commits_request_but_does_not_infer_closure() {
    let root = Temp::new();
    let (store, intent, ids) = stored(&root);
    let mut manager = FakeManager::new(&root);
    let held = prepare_only(&store, &mut manager, &ids).prepared.unwrap();
    let cancel = cancellation(&store, &ids);
    let receipt = cancel_prepared(&store, &key(), &mut manager, held, cancel.clone()).unwrap();
    manager.finish();
    assert!(!receipt.duplicate);
    assert_eq!(manager.stopped, 1);
    let restored = store.load(&key(), Access::ReadOnly).unwrap();
    let r = restored.domains().unwrap().domain(&intent.id).unwrap();
    assert!(r.cancelled && r.binding_observation.is_some() && r.closure.is_none());
    assert_eq!(r.outcome, HistoricalOutcome::Unknown);
    assert!(r.holds(cortex_core::program::scheduling::Resource::Workers));
    assert!(
        cancel_domain(&store, &key(), &mut manager, cancel)
            .unwrap()
            .duplicate
    );
    assert_eq!(manager.stopped, 1); // A repeated receipt cannot repeat StopUnit.
}

#[test]
fn explicit_release_refuses_revoked_cancelled_expired_and_wrong_store_handles() {
    use cortex_core::program::operational::EffectEvent;
    for case in 0..4 {
        let root = Temp::new();
        let (store, intent, ids) = stored(&root);
        let other_root = Temp::new();
        let (other_store, _, _) = stored(&other_root);
        let mut manager = FakeManager::new(&root);
        let mut held = prepare_only(&store, &mut manager, &ids).prepared.unwrap();
        if case == 0 {
            let k = store.load(&key(), Access::ReadOnly).unwrap();
            let revoke = fixture::domain_operation(
                &k,
                "revoke-after-bind",
                Command::SupervisedEffect(Box::new(EffectEvent::Revoke {
                    grant: intent.effect.grant.clone(),
                    at: k.domains().unwrap().last_tick(),
                    basis: fixture::source("revoke"),
                })),
            );
            store.admit(&key(), revoke).unwrap();
        } else if case == 1 {
            store.admit(&key(), cancellation(&store, &ids)).unwrap();
        } else if case == 2 {
            held.end = Instant::now(); // Deterministic expired-clock input; no scheduling sleep.
        }
        let target = if case == 3 { &other_store } else { &store };
        let result = release(target, &key(), &mut manager, held, &AtomicBool::new(false));
        manager.finish();
        assert!(result.diagnostic.is_some(), "case {case}");
        assert!(
            result.release.is_none() && result.nonce.is_none(),
            "case {case}"
        );
        assert_eq!(manager.created, 1);
        assert_eq!(manager.stopped, 0);
        assert_eq!(
            store
                .load(&key(), Access::ReadOnly)
                .unwrap()
                .domains()
                .unwrap()
                .domain(&intent.id)
                .unwrap()
                .phase,
            DomainPhase::Bound
        );
    }
}

#[test]
fn stale_membership_at_locked_release_retains_uncertainty_without_transmission() {
    let root = Temp::new();
    let (store, intent, ids) = stored(&root);
    let mut manager = FakeManager::new(&root);
    let held = prepare_only(&store, &mut manager, &ids).prepared.unwrap();
    manager.change_on_observation = Some(manager.observed + 2); // The in-lock observation, after the precheck.
    let result = release(&store, &key(), &mut manager, held, &AtomicBool::new(false));
    manager.finish();
    assert_eq!(
        result.diagnostic.unwrap().cause,
        DomainCause::Correspondence
    );
    assert!(result.release.is_some() && result.nonce.is_none());
    let k = store.load(&key(), Access::ReadOnly).unwrap();
    let r = k.domains().unwrap().domain(&intent.id).unwrap();
    assert_eq!(r.phase, DomainPhase::ReleaseUncertain);
    assert_eq!(r.outcome, HistoricalOutcome::Unknown);
    assert!(r.holds(cortex_core::program::scheduling::Resource::Workers));
}

#[test]
fn normal_prepared_cancel_fixture_closure_allows_distinct_next_prepare_only_with_measurement() {
    for measured in [Some(1), None] {
        let root = Temp::new();
        let (store, intent, ids) = stored(&root);
        let mut manager = FakeManager::new(&root);
        let held = prepare_only(&store, &mut manager, &ids).prepared.unwrap();
        cancel_prepared(
            &store,
            &key(),
            &mut manager,
            held,
            cancellation(&store, &ids),
        )
        .unwrap();
        manager.finish();
        observe_closure(
            &store,
            &key(),
            &mut manager,
            &ids,
            OperationId::new("normal-partial-readback").unwrap(),
        )
        .unwrap();
        let mut k = store.load(&key(), Access::ReadOnly).unwrap();
        let retained = k.domains().unwrap().domain(&intent.id).unwrap();
        assert_eq!(retained.closure_observations.len(), 1);
        assert!(retained.holds(cortex_core::program::scheduling::Resource::Workers));
        assert!(retained.unresolved_consumption(Consumption::CpuExecutionMillis));
        assert!(
            retained
                .closure_inspection()
                .missing
                .contains(&ClosureProofGap::StreamCoverage)
        );
        let previous = k.revision();
        let at = k.domains().unwrap().last_tick();
        // An explicitly controlled fixture supplies its own closure oracle.
        // This is not evidence that the systemd observer can establish closure.
        fixture::event(
            &mut k,
            "fixture-close",
            DomainEvent::Closed {
                receipt: fixture::closure(&intent, measured),
                at,
            },
        );
        let mut next = fixture::add_member(&mut k, "normal-next");
        next.pool = intent.pool.clone();
        for input in &k.operations()[previous as usize..] {
            store.admit(&key(), input.clone()).unwrap();
        }
        let reserve = fixture::domain_operation(
            &k,
            "next-reserve",
            Command::Domain(Box::new(DomainEvent::Reserve {
                intent: next.clone(),
                at,
            })),
        );
        if measured.is_none() {
            assert!(store.admit(&key(), reserve).is_err());
            continue;
        }
        store.admit(&key(), reserve).unwrap();
        let next_root = Temp::new();
        let mut next_manager = FakeManager::new(&next_root);
        let mut next_ids = ids;
        next_ids.domain = next.id;
        next_ids.create = OperationId::new("second-create").unwrap();
        next_ids.bind = OperationId::new("second-bind").unwrap();
        next_ids.release = OperationId::new("second-release").unwrap();
        next_ids.report = OperationId::new("second-report").unwrap();
        next_ids.at = at;
        let held = prepare_only(&store, &mut next_manager, &next_ids)
            .prepared
            .unwrap();
        let next_cancel = {
            let k = store.load(&key(), Access::ReadOnly).unwrap();
            fixture::domain_operation(
                &k,
                "second-cancel",
                Command::Domain(Box::new(DomainEvent::Cancel {
                    domain: next_ids.domain.clone(),
                    at: k.domains().unwrap().last_tick(),
                    basis: fixture::source("second-cancel"),
                })),
            )
        };
        cancel_prepared(&store, &key(), &mut next_manager, held, next_cancel).unwrap();
        next_manager.finish();
        assert_eq!(manager.created, 1);
        assert_eq!(next_manager.created, 1);
        let k = store.load(&key(), Access::ReadOnly).unwrap();
        let first = k.domains().unwrap().domain(&intent.id).unwrap();
        assert_eq!(first.outcome, HistoricalOutcome::Unknown);
        assert_eq!(
            first.measured_consumption(Consumption::CpuExecutionMillis),
            Some(1)
        );
        assert!(first.holds(cortex_core::program::scheduling::Resource::DiskBytes));
        let second = k.domains().unwrap().domain(&next_ids.domain).unwrap();
        assert!(second.cancelled && second.closure.is_none());
        assert!(second.holds(cortex_core::program::scheduling::Resource::Workers));
    }
}

#[test]
fn post_cancel_observation_is_read_only_partial_and_never_settles_resources() {
    let root = Temp::new();
    let (store, intent, ids) = stored(&root);
    let mut manager = FakeManager::new(&root);
    let held = prepare_only(&store, &mut manager, &ids).prepared.unwrap();
    let observations = manager.observed;
    assert!(
        observe_closure(
            &store,
            &key(),
            &mut manager,
            &ids,
            OperationId::new("before-cancel").unwrap()
        )
        .is_err()
    );
    assert_eq!(manager.observed, observations);
    cancel_prepared(
        &store,
        &key(),
        &mut manager,
        held,
        cancellation(&store, &ids),
    )
    .unwrap();
    manager.finish();
    let created = manager.created;
    let stopped = manager.stopped;
    observe_closure(
        &store,
        &key(),
        &mut manager,
        &ids,
        OperationId::new("post-cancel").unwrap(),
    )
    .unwrap();
    assert_eq!(manager.created, created);
    assert_eq!(manager.stopped, stopped);
    let before = store.load(&key(), Access::ReadOnly).unwrap();
    let r = before.domains().unwrap().domain(&intent.id).unwrap();
    assert_eq!(r.closure_observations.len(), 1);
    assert!(r.closure_observations[0].populated);
    assert!(
        r.closure_inspection()
            .missing
            .contains(&ClosureProofGap::RemainingMembers)
    );
    assert!(r.closure.is_none());
    assert!(r.holds(cortex_core::program::scheduling::Resource::Workers));
    assert!(r.unresolved_consumption(Consumption::CpuExecutionMillis));
    assert_eq!(r.outcome, HistoricalOutcome::Unknown);
    let restored = store.load(&key(), Access::ReadOnly).unwrap();
    assert!(before.domains() == restored.domains());
    assert_eq!(manager.created, 1);
    assert_eq!(manager.stopped, 1);
}

#[test]
fn post_cancel_replaced_manager_identity_or_invalid_authority_cannot_publish_readback() {
    for replaced in [true, false] {
        let root = Temp::new();
        let (store, _, mut ids) = stored(&root);
        let mut manager = FakeManager::new(&root);
        let held = prepare_only(&store, &mut manager, &ids).prepared.unwrap();
        cancel_prepared(
            &store,
            &key(),
            &mut manager,
            held,
            cancellation(&store, &ids),
        )
        .unwrap();
        manager.finish();
        if replaced {
            manager.replace = true;
        } else {
            ids.grant.baseline = cortex_core::binding::ContractRevision::new(2).unwrap();
        }
        let before = store.load(&key(), Access::ReadOnly).unwrap();
        assert!(
            observe_closure(
                &store,
                &key(),
                &mut manager,
                &ids,
                OperationId::new("refused-readback").unwrap()
            )
            .is_err()
        );
        let after = store.load(&key(), Access::ReadOnly).unwrap();
        assert_eq!(before.revision(), after.revision());
        assert!(before.domains() == after.domains());
        assert_eq!(manager.created, 1);
        assert_eq!(manager.stopped, 1);
    }
}
