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
    observed: usize,
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
            observed: 0,
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
        if self.replace && self.observed > 1 {
            identity.invocation = "replacement".into();
        }
        Ok(ManagedObservation {
            identity,
            main_pid: std::process::id(),
            start_ticks: crate::domain_bootstrap::ticks(std::process::id()).unwrap(),
            populated: true,
            cpu_micros: None,
            memory_peak: None,
            restart_prevented: true,
        })
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
