//! SOURCE_SIMULATION: canonical store cuts with typed manager snapshots and
//! finite in-process nonce bootstrap threads. No bus, service, cgroup or child
//! payload is activated; these tests do not establish host lifetime guarantees.
use super::*;
use crate::ServiceLimits;
use crate::native_execution::managed::{ManagerLifecycleReadback, ManagerObservationError};
use cortex_core::program::scheduling::Resource;

struct LifecycleManager {
    inner: FakeManager,
    readback: ManagerLifecycleReadback,
    lost_completion: bool,
    lost_stop: bool,
    completions: usize,
    structured_stdout: Option<&'static [u8]>,
}
impl LifecycleManager {
    fn new(root: &Temp, intent: &DomainIntent) -> Self {
        Self {
            inner: FakeManager::new(root),
            readback: ManagerLifecycleReadback {
                identity: fixture::identity(intent),
                service_type: Some("exec".into()),
                restart: Some("no".into()),
                kill_mode: Some("control-group".into()),
                controls: Some(fixture::binding(intent).controls),
                active_enter_monotonic_us: Some(100),
            },
            lost_completion: false,
            lost_stop: false,
            completions: 0,
            structured_stdout: None,
        }
    }
    fn check_readback(
        &self,
        intent: &DomainIntent,
        actual: Option<DomainIdentity>,
    ) -> Result<(), DomainDiagnostic> {
        let mut readback = self.readback.clone();
        if let Some(actual) = actual {
            readback.identity = actual;
        }
        readback
            .check(
                &fixture::identity(intent),
                &ServiceLimits {
                    wall_ms: 1000,
                    cpu_quota_us: 100000,
                    cpu_period_us: 100000,
                    memory_bytes: 512,
                    swap_bytes: 0,
                    tasks: 8,
                },
                Some(1000100),
            )
            .map_err(|e| {
                issue(
                    DomainPhaseDiagnostic::Preparation,
                    if e == ManagerObservationError::DeadlineMismatch {
                        DomainCause::Deadline
                    } else {
                        DomainCause::Correspondence
                    },
                )
            })
    }
}
impl DomainManager for LifecycleManager {
    fn check_context(
        &self,
        _: &CoordinationKernel,
        intent: &DomainIntent,
        _: u64,
    ) -> Result<(), DomainDiagnostic> {
        self.check_readback(intent, None)
    }
    fn corresponds(&self, intent: &DomainIntent) -> Result<(), DomainDiagnostic> {
        self.inner.corresponds(intent)
    }
    fn create(&mut self, intent: &DomainIntent, end: Instant) -> Result<(), DomainDiagnostic> {
        self.inner.create(intent, end)
    }
    fn observe(
        &mut self,
        intent: &DomainIntent,
        end: Instant,
    ) -> Result<ManagedObservation, DomainDiagnostic> {
        let observed = self.inner.observe(intent, end)?;
        self.check_readback(intent, Some(observed.identity.clone()))?;
        Ok(observed)
    }
    fn observe_tree(
        &mut self,
        intent: &DomainIntent,
        end: Instant,
    ) -> Result<(ManagedObservation, Vec<DomainCgroupObservation>), DomainDiagnostic> {
        let observed = self.observe(intent, end)?;
        let root = DomainCgroupObservation {
            relative_path: String::new(),
            device: observed.identity.cgroup_device,
            inode: observed.identity.cgroup_inode,
            populated: observed.populated,
            members: observed.direct_members.clone(),
        };
        Ok((observed, vec![root]))
    }
    fn stop(
        &mut self,
        intent: &DomainIntent,
        expected: &DomainIdentity,
        end: Instant,
    ) -> Result<(), DomainDiagnostic> {
        self.inner.stop(intent, expected, end)?;
        if self.lost_stop {
            Err(issue(
                DomainPhaseDiagnostic::Closure,
                DomainCause::ManagerRefusal,
            ))
        } else {
            Ok(())
        }
    }
    fn finish(
        &mut self,
        channel: &mut UnixStream,
        context: DomainFinish<'_>,
        authority: &mut dyn FnMut() -> Result<(), DomainDiagnostic>,
    ) -> Result<cortex_core::program::operational::EffectOutcome, DomainDiagnostic> {
        authority()?;
        let reply = frame_read(channel, context.end)
            .map_err(|e| issue(DomainPhaseDiagnostic::Release, DomainCause::Io(e.kind())))?;
        if reply != json!([1, "nonce", context.intent.request.as_str(), context.value]) {
            return Err(issue(DomainPhaseDiagnostic::Release, DomainCause::Protocol));
        }
        self.completions += 1;
        authority()?;
        if self.lost_completion {
            return Err(issue(
                DomainPhaseDiagnostic::Release,
                DomainCause::ManagerRefusal,
            ));
        }
        self.check_readback(context.intent, None)?;
        if let Some(bytes) = self.structured_stdout {
            use crate::structured_output::{BoundStructuredObservation, StructuredObserver};
            use cortex_core::program::supervision::{ProcessLimits, TransportLimits};
            // SOURCE_SIMULATION: a distinct fake stdout transport, not a nonce
            // control frame and not a schema claiming application authority.
            let limits = ProcessLimits {
                wall_ms: 1000,
                cleanup_ms: 100,
                input_bytes: 1024,
                output_bytes: 1024,
                frame_bytes: 64,
                authority_poll_ms: 1,
                executable_bytes: 4096,
                arguments: 0,
                argument_bytes: 0,
                transport: Some(TransportLimits {
                    startup_ms: 100,
                    idle_ms: 200,
                    stdout_bytes: 1024,
                    stderr_bytes: 1024,
                    records: 8,
                    buffer_bytes: 16384,
                }),
            };
            let invalid = |_| issue(DomainPhaseDiagnostic::Release, DomainCause::Protocol);
            let mut observer = StructuredObserver::new(&limits, 0, 0).map_err(invalid)?;
            observer
                .feed(bytes, 1)
                .and_then(|_| observer.finish(2))
                .map_err(invalid)?;
            let bound = BoundStructuredObservation::new(
                context.intent,
                context.binding,
                context.release,
                observer.summary(),
            )
            .map_err(invalid)?;
            bound
                .check(context.intent, context.binding, context.release)
                .map_err(invalid)?;
            authority()?;
        }
        Ok(cortex_core::program::operational::EffectOutcome::Succeeded)
    }
}
fn prepare_lifecycle(
    store: &ProgramStore,
    manager: &mut LifecycleManager,
    ids: &DomainOperations,
) -> DomainPreparation {
    let endpoint = manager.inner.endpoint.clone();
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
fn execute_lifecycle(
    store: &ProgramStore,
    manager: &mut LifecycleManager,
    ids: &DomainOperations,
) -> DomainRun {
    let prepared = prepare_lifecycle(store, manager, ids);
    match prepared.prepared {
        Some(held) => release(store, &key(), manager, held, &AtomicBool::new(false)),
        None => prepared.run,
    }
}
fn count_event(kernel: &CoordinationKernel, domain: &DomainId, kind: u8) -> usize {
    kernel
        .operations()
        .iter()
        .filter(|op| {
            matches!(&op.command, Command::Domain(event) if match event.as_ref() {
                DomainEvent::CreateRequested { domain: found, .. } => kind == 0 && found == domain,
                DomainEvent::BoundObserved { domain: found, .. } => kind == 1 && found == domain,
                DomainEvent::ReleaseRequested { domain: found, .. } => kind == 2 && found == domain,
                DomainEvent::Report { domain: found, .. } => kind == 3 && found == domain,
                _ => false,
            })
        })
        .count()
}
fn assert_retained(kernel: &CoordinationKernel, intent: &DomainIntent) {
    let record = kernel.domains().unwrap().domain(&intent.id).unwrap();
    assert_eq!(record.outcome, HistoricalOutcome::Unknown);
    assert!(record.closure.is_none());
    assert!(record.holds(Resource::WorkerSlots));
    assert!(record.holds(Resource::DiskBytes));
    assert!(record.unresolved_consumption(Consumption::CpuExecutionMillis));
}

#[test]
fn missing_manager_facts_refuse_before_create_listener_or_journal_cut() {
    for fault in 0..6 {
        let root = Temp::new();
        let (store, intent, ids) = stored(&root);
        let before = store.load(&key(), Access::ReadOnly).unwrap().revision();
        let mut manager = LifecycleManager::new(&root, &intent);
        match fault {
            0 => manager.readback.controls = None,
            1 => manager.readback.service_type = None,
            2 => {
                manager
                    .readback
                    .controls
                    .as_mut()
                    .unwrap()
                    .remain_after_exit = false
            }
            3 => {
                manager
                    .readback
                    .controls
                    .as_mut()
                    .unwrap()
                    .timeout_stop_micros = 0
            }
            4 => manager.readback.active_enter_monotonic_us = None,
            _ => manager.readback.active_enter_monotonic_us = Some(101),
        }
        let start = Instant::now();
        let result = prepare(
            &store,
            &key(),
            &mut manager,
            ExecutionRequest {
                ids: &ids,
                nonce: "public-nonce",
                cancel: &AtomicBool::new(false),
                start,
                end: start + Duration::from_secs(1),
            },
            || {
                panic!(
                    "an unconditional manager-guarantees success would reach this unauthorized listener cut"
                )
            },
        );
        assert!(
            result.run.diagnostic.is_some()
                && result.run.creation.is_none()
                && result.prepared.is_none()
        );
        assert_eq!(manager.inner.created, 0);
        assert!(!manager.inner.endpoint.exists());
        assert_eq!(
            store.load(&key(), Access::ReadOnly).unwrap().revision(),
            before
        );
    }
}

#[test]
fn changed_incarnation_before_and_after_release_cut_preserves_exact_committed_prefix() {
    for in_locked_cut in [false, true] {
        let root = Temp::new();
        let (store, intent, ids) = stored(&root);
        let mut manager = LifecycleManager::new(&root, &intent);
        let held = prepare_lifecycle(&store, &mut manager, &ids)
            .prepared
            .unwrap();
        manager.inner.change_on_observation =
            Some(manager.inner.observed + if in_locked_cut { 2 } else { 1 });
        let result = release(&store, &key(), &mut manager, held, &AtomicBool::new(false));
        manager.inner.finish();
        assert_eq!(
            result.diagnostic.unwrap().cause,
            DomainCause::Correspondence
        );
        assert!(result.nonce.is_none() && result.report.is_none());
        let kernel = store.load(&key(), Access::ReadOnly).unwrap();
        assert_eq!(count_event(&kernel, &intent.id, 0), 1);
        assert_eq!(count_event(&kernel, &intent.id, 1), 1);
        assert_eq!(
            count_event(&kernel, &intent.id, 2),
            usize::from(in_locked_cut)
        );
        assert_eq!(count_event(&kernel, &intent.id, 3), 0);
        assert_retained(&kernel, &intent);
        assert_eq!(manager.completions, 0);
        assert_eq!(manager.inner.created, 1);
    }
}

// A larger explicitly synthetic pool allows two distinct fixture members. This
// changes no production limit and supplies no physical enforcement claim.
fn stored_neighbors(
    root: &Temp,
) -> (
    ProgramStore,
    DomainIntent,
    DomainOperations,
    DomainIntent,
    DomainOperations,
) {
    let mut kernel = CoordinationKernel::new_domain_fixture(fixture::definition()).unwrap();
    let mut pool = fixture::pool();
    pool.identity.boot = fs::read_to_string("/proc/sys/kernel/random/boot_id")
        .unwrap()
        .trim()
        .into();
    for capacity in &mut pool.capacities {
        if capacity.resource == Resource::WorkerSlots {
            capacity.available = Some(3);
        }
        if matches!(capacity.resource, Resource::MemoryBytes(_)) {
            capacity.resource = fixture::memory_resource_for(pool.identity.clone());
        }
    }
    fixture::event(&mut kernel, "pool", DomainEvent::ConfigurePool(pool));
    let first = fixture::setup(&mut kernel, "A");
    let neighbor = fixture::add_member(&mut kernel, "lifecycle-neighbor");
    for (name, intent) in [("reserve-first", &first), ("reserve-neighbor", &neighbor)] {
        fixture::event(
            &mut kernel,
            name,
            DomainEvent::Reserve {
                intent: intent.clone(),
                at: 1,
            },
        );
    }
    let ids = |intent: &DomainIntent, prefix: &str| DomainOperations {
        domain: intent.id.clone(),
        create: OperationId::new(&format!("{prefix}-create")).unwrap(),
        bind: OperationId::new(&format!("{prefix}-bind")).unwrap(),
        release: OperationId::new(&format!("{prefix}-release")).unwrap(),
        report: OperationId::new(&format!("{prefix}-report")).unwrap(),
        grant: fixture::domain_operation(
            &kernel,
            "unused",
            Command::Domain(Box::new(DomainEvent::Unknown {
                domain: intent.id.clone(),
                at: 2,
                basis: fixture::source("unused"),
            })),
        )
        .grant,
        basis: fixture::source("typed-manager-source-simulation"),
        at: 2,
    };
    let first_ids = ids(&first, "lifecycle-first");
    let neighbor_ids = ids(&neighbor, "lifecycle-neighbor");
    let store = ProgramStore::create_domain_fixture(
        &root.0.join("store"),
        &key(),
        kernel.initial().clone(),
        ReadLimits::new(16000000, Duration::from_secs(5)).unwrap(),
    )
    .unwrap();
    for op in kernel.operations() {
        store.admit(&key(), op.clone()).unwrap();
    }
    (store, first, first_ids, neighbor, neighbor_ids)
}

#[test]
fn lost_creation_or_completion_reply_never_replays_and_leaves_valid_neighbor_unchanged() {
    for lost_create in [true, false] {
        let root = Temp::new();
        let (store, intent, ids, neighbor, mut neighbor_ids) = stored_neighbors(&root);
        let first_root = Temp::new();
        let mut manager = LifecycleManager::new(&first_root, &intent);
        manager.inner.lost = lost_create;
        manager.lost_completion = !lost_create;
        let result = execute_lifecycle(&store, &mut manager, &ids);
        manager.inner.finish();
        assert_eq!(
            result.diagnostic.unwrap().cause,
            DomainCause::ManagerRefusal
        );
        let before = store.load(&key(), Access::ReadOnly).unwrap();
        assert_retained(&before, &intent);
        assert_eq!(count_event(&before, &intent.id, 0), 1);
        assert_eq!(
            count_event(&before, &intent.id, 2),
            usize::from(!lost_create)
        );
        assert_eq!(count_event(&before, &intent.id, 3), 0);
        let original = before
            .domains()
            .unwrap()
            .domain(&intent.id)
            .unwrap()
            .clone();
        let reopened = ProgramStore::open(
            &root.0.join("store"),
            &key(),
            Access::ReadOnly,
            ReadLimits::new(16000000, Duration::from_secs(5)).unwrap(),
        )
        .unwrap();
        let revision = reopened.load(&key(), Access::ReadOnly).unwrap().revision();
        let next = execute_lifecycle(&reopened, &mut manager, &ids);
        assert!(next.recovery_only && next.nonce.is_none());
        assert_eq!(manager.inner.created, 1);
        assert_eq!(manager.completions, usize::from(!lost_create));
        assert_eq!(
            reopened.load(&key(), Access::ReadOnly).unwrap().revision(),
            revision
        );
        let neighbor_root = Temp::new();
        let mut neighbor_manager = LifecycleManager::new(&neighbor_root, &neighbor);
        // This is a fresh neighbor operation, not a retry or generation refresh.
        // Its clock starts at the current canonical tick after the prior cut.
        neighbor_ids.at = store
            .load(&key(), Access::ReadOnly)
            .unwrap()
            .domains()
            .unwrap()
            .last_tick();
        assert_ne!(neighbor.effect.scope.program, intent.effect.scope.program);
        assert_eq!(neighbor.effect.scope.task.as_str(), "A");
        let valid = execute_lifecycle(&store, &mut neighbor_manager, &neighbor_ids);
        neighbor_manager.inner.finish();
        assert!(valid.diagnostic.is_none(), "{:?}", valid.diagnostic);
        assert_eq!(valid.nonce.as_deref(), Some("public-nonce"));
        let after = store.load(&key(), Access::ReadOnly).unwrap();
        assert!(after.domains().unwrap().domain(&intent.id).unwrap() == &original);
        assert_eq!(neighbor_manager.inner.created, 1);
        assert_eq!(neighbor_manager.completions, 1);
    }
}

#[test]
fn lost_stop_acknowledgment_is_not_repeated_or_converted_to_closure_or_refund() {
    let root = Temp::new();
    let (store, intent, ids, neighbor, mut neighbor_ids) = stored_neighbors(&root);
    let first_root = Temp::new();
    let mut manager = LifecycleManager::new(&first_root, &intent);
    let held = prepare_lifecycle(&store, &mut manager, &ids)
        .prepared
        .unwrap();
    manager.lost_stop = true;
    let cancel = cancellation(&store, &ids);
    let failure = cancel_prepared(&store, &key(), &mut manager, held, cancel.clone()).unwrap_err();
    manager.inner.finish();
    assert_eq!(failure.cause, DomainCause::ManagerRefusal);
    assert_eq!(manager.inner.stopped, 1);
    assert!(
        cancel_domain(&store, &key(), &mut manager, cancel)
            .unwrap()
            .duplicate
    );
    assert_eq!(manager.inner.stopped, 1);
    observe_closure(
        &store,
        &key(),
        &mut manager,
        &ids,
        OperationId::new("lifecycle-partial-closure").unwrap(),
    )
    .unwrap();
    let before = store.load(&key(), Access::ReadOnly).unwrap();
    assert_retained(&before, &intent);
    let original = before
        .domains()
        .unwrap()
        .domain(&intent.id)
        .unwrap()
        .clone();
    assert!(original.cancelled && original.closure.is_none());
    assert!(original.closure_observations[0].populated);
    let neighbor_root = Temp::new();
    let mut neighbor_manager = LifecycleManager::new(&neighbor_root, &neighbor);
    // This is a fresh neighbor operation, not a retry or generation refresh.
    // Its clock starts at the current canonical tick after the prior cut.
    neighbor_ids.at = store
        .load(&key(), Access::ReadOnly)
        .unwrap()
        .domains()
        .unwrap()
        .last_tick();
    assert_ne!(neighbor.effect.scope.program, intent.effect.scope.program);
    assert_eq!(neighbor.effect.scope.task.as_str(), "A");
    let valid = execute_lifecycle(&store, &mut neighbor_manager, &neighbor_ids);
    neighbor_manager.inner.finish();
    assert!(valid.diagnostic.is_none(), "{:?}", valid.diagnostic);
    let after = store.load(&key(), Access::ReadOnly).unwrap();
    assert!(after.domains().unwrap().domain(&intent.id).unwrap() == &original);
    assert_eq!(manager.inner.stopped, 1);
    assert_eq!(manager.inner.created, 1);
    assert_eq!(count_event(&after, &intent.id, 2), 0);
    assert_eq!(count_event(&after, &intent.id, 3), 0);
}

#[test]
fn structured_observation_binds_full_context_without_journal_or_neighbor_effects() {
    use crate::structured_output::{BoundStructuredObservation, StructuredSummary};
    use cortex_core::program::supervision::InterruptionReason;
    let root = Temp::new();
    let (store, intent, ids) = stored(&root);
    let before = store.load(&key(), Access::ReadOnly).unwrap().revision();
    let binding = fixture::binding(&intent);
    let summary = StructuredSummary {
        records: 1,
        first_record_ms: Some(1),
        interruption: None,
        stdout_finished: true,
    };
    let bound =
        BoundStructuredObservation::new(&intent, &binding, &ids.release, summary.clone()).unwrap();
    assert_eq!(bound.check(&intent, &binding, &ids.release), Ok(()));
    let (_, neighbor_intent) = fixture::coordinator();
    let neighbor_binding = fixture::binding(&neighbor_intent);
    let neighbor_release = OperationId::new("independent-stream-release").unwrap();
    let neighbor = BoundStructuredObservation::new(
        &neighbor_intent,
        &neighbor_binding,
        &neighbor_release,
        summary,
    )
    .unwrap();
    for change in 0..6 {
        let mut changed_intent = intent.clone();
        let mut changed_binding = binding.clone();
        let mut release = ids.release.clone();
        match change {
            0 => changed_intent.request = OperationId::new("different-request").unwrap(),
            1 => changed_intent.effect.scope.generation = fixture::rev(2),
            2 => {
                changed_intent.effect.scope.package =
                    cortex_core::program::PackageId::new("different-package").unwrap()
            }
            3 => changed_intent.configuration = fixture::source("different-configuration"),
            4 => changed_binding.identity.invocation = "different-manager-incarnation".into(),
            _ => release = OperationId::new("different-release").unwrap(),
        }
        assert_eq!(
            bound.check(&changed_intent, &changed_binding, &release),
            Err(InterruptionReason::AuthorityLost)
        );
        assert_eq!(
            neighbor.check(&neighbor_intent, &neighbor_binding, &neighbor_release),
            Ok(())
        );
        assert_eq!(
            store.load(&key(), Access::ReadOnly).unwrap().revision(),
            before
        );
    }
}

#[test]
fn malformed_structured_stream_refuses_canonical_report_without_replay_or_neighbor_damage() {
    let root = Temp::new();
    let (store, intent, ids, neighbor, mut neighbor_ids) = stored_neighbors(&root);
    let first_root = Temp::new();
    let mut manager = LifecycleManager::new(&first_root, &intent);
    manager.structured_stdout = Some(b"{");
    let failed = execute_lifecycle(&store, &mut manager, &ids);
    manager.inner.finish();
    assert_eq!(failed.diagnostic.unwrap().cause, DomainCause::Protocol);
    assert!(failed.report.is_none() && failed.nonce.is_none());
    let before = store.load(&key(), Access::ReadOnly).unwrap();
    assert_retained(&before, &intent);
    assert_eq!(count_event(&before, &intent.id, 0), 1);
    assert_eq!(count_event(&before, &intent.id, 2), 1);
    assert_eq!(count_event(&before, &intent.id, 3), 0);
    let original = before
        .domains()
        .unwrap()
        .domain(&intent.id)
        .unwrap()
        .clone();
    assert!(execute_lifecycle(&store, &mut manager, &ids).recovery_only);
    assert_eq!(manager.inner.created, 1);
    assert_eq!(manager.completions, 1);
    let neighbor_root = Temp::new();
    let mut valid_manager = LifecycleManager::new(&neighbor_root, &neighbor);
    valid_manager.structured_stdout = Some(b"null\n");
    neighbor_ids.at = store
        .load(&key(), Access::ReadOnly)
        .unwrap()
        .domains()
        .unwrap()
        .last_tick();
    let valid = execute_lifecycle(&store, &mut valid_manager, &neighbor_ids);
    valid_manager.inner.finish();
    assert!(valid.diagnostic.is_none(), "{:?}", valid.diagnostic);
    assert!(valid.report.is_some());
    let after = store.load(&key(), Access::ReadOnly).unwrap();
    assert_eq!(count_event(&after, &neighbor.id, 3), 1);
    assert!(after.domains().unwrap().domain(&intent.id).unwrap() == &original);
}

#[test]
fn source_graph_accepted_dependencies_do_not_settle_execution_debt() {
    use cortex_core::program::operational::{EffectEvent, EffectMode, Policy};
    let root = Temp::new();
    let mut kernel = CoordinationKernel::new_domain_fixture(fixture::definition()).unwrap();
    let mut pool = fixture::pool();
    pool.identity.boot = fs::read_to_string("/proc/sys/kernel/random/boot_id")
        .unwrap()
        .trim()
        .into();
    // Explicit finite logical fixture allowance retains all three Unknown
    // reservations; no closure or physical resource enforcement is asserted.
    for capacity in &mut pool.capacities {
        if capacity.resource == Resource::WorkerSlots {
            capacity.available = Some(4);
        }
        if matches!(capacity.resource, Resource::MemoryBytes(_)) {
            capacity.resource = fixture::memory_resource_for(pool.identity.clone());
            capacity.available = Some(2048);
        }
    }
    pool.consumption[0].total = 200;
    fixture::event(
        &mut kernel,
        "graph-pool",
        DomainEvent::ConfigurePool(pool.clone()),
    );
    fixture::submit(
        &mut kernel,
        "graph-member-policy",
        Command::SupervisedEffect(Box::new(EffectEvent::Policy(Policy {
            reference: fixture::source("domain-member-policy"),
            program: fixture::definition().id().clone(),
            baseline: fixture::rev(1),
            mode: EffectMode::SupervisedFixture,
            host: fixture::source("controlled-host"),
            privacy: fixture::source("synthetic-only"),
            capacities: pool.capacities.clone(),
            cumulative: vec![],
            max_effects: 16,
            max_retries: 0,
        }))),
    );
    // Existing canonical graph: AB, DE are upstream; B and E are independent;
    // F requires both BF and EF. These are synthetic accepted source artifacts.
    fixture::release(&mut kernel, "AB");
    fixture::release(&mut kernel, "DE");
    let b = fixture::setup(&mut kernel, "B");
    let e = fixture::setup(&mut kernel, "E");
    for (name, intent) in [("graph-reserve-B", &b), ("graph-reserve-E", &e)] {
        fixture::event(
            &mut kernel,
            name,
            DomainEvent::Reserve {
                intent: intent.clone(),
                at: 1,
            },
        );
    }
    let store = ProgramStore::create_domain_fixture(
        &root.0.join("store"),
        &key(),
        kernel.initial().clone(),
        ReadLimits::new(16000000, Duration::from_secs(5)).unwrap(),
    )
    .unwrap();
    for op in kernel.operations() {
        store.admit(&key(), op.clone()).unwrap();
    }
    let make_ids = |intent: &DomainIntent, label: &str| {
        let current = store.load(&key(), Access::ReadOnly).unwrap();
        DomainOperations {
            domain: intent.id.clone(),
            create: OperationId::new(&format!("graph-{label}-create")).unwrap(),
            bind: OperationId::new(&format!("graph-{label}-bind")).unwrap(),
            release: OperationId::new(&format!("graph-{label}-release")).unwrap(),
            report: OperationId::new(&format!("graph-{label}-report")).unwrap(),
            grant: fixture::domain_operation(
                &current,
                "unused",
                Command::Domain(Box::new(DomainEvent::Unknown {
                    domain: intent.id.clone(),
                    at: 2,
                    basis: fixture::source("unused"),
                })),
            )
            .grant,
            basis: fixture::source("source-simulation-graph"),
            at: current.domains().unwrap().last_tick(),
        }
    };
    let assert_f_held = || {
        use cortex_core::program::ActorId;
        use cortex_core::program::operational::OperationalGrant;
        use cortex_core::program::{PackageId, context::PackageRequest};
        let current = store.load(&key(), Access::ReadOnly).unwrap();
        let revision = current.revision();
        // A non-durable clone exercises the actual claim/package/requested-grant
        // protocol. It does not create F authority or a domain in the real store.
        let mut probe = current.clone();
        let task = fixture::task("F");
        let candidate = probe
            .state()
            .readiness()
            .scope(&task)
            .unwrap()
            .context()
            .contract()
            .candidate()
            .clone();
        let generation = probe.state().readiness().scope(&task).unwrap().generation();
        let scope = probe.state().readiness().scope(&task).unwrap();
        let notify = fixture::notify(
            probe.state(),
            "ready-F",
            cortex_core::parallel::Event::Preconditions {
                scope,
                ready: true,
                source: fixture::source("source-simulation-preconditions"),
            },
        );
        fixture::submit(&mut probe, "ready-F", Command::Notify(notify));
        let attempt = fixture::attempt("attempt-F", "F", "operator");
        fixture::submit(&mut probe, "claim-F", Command::Claim(attempt.clone()));
        let package = PackageRequest {
            id: PackageId::new("package-F").unwrap(),
            revision: fixture::rev(1),
            task: task.clone(),
            baseline: fixture::rev(1),
            generation,
            sources: vec![],
            methods: vec![],
            omissions: "Synthetic owned fixture".into(),
            deliverable: "Provider-free source simulation".into(),
            stop_rules: "Hold on uncertainty".into(),
        };
        fixture::submit(&mut probe, "package-F", Command::Package(package.clone()));
        let mut effect_scope = b.effect.scope.clone();
        effect_scope.task = task;
        effect_scope.candidate = candidate;
        effect_scope.attempt = attempt.id;
        effect_scope.generation = generation;
        effect_scope.package = package.id;
        effect_scope.package_revision = package.revision;
        let requested = OperationalGrant {
            id: cortex_core::program::GrantId::new("operational-F").unwrap(),
            principal: ActorId::new("operator").unwrap(),
            policy: fixture::source("domain-member-policy"),
            scope: effect_scope,
            not_before: 1,
            expires_before: 1000,
            ceilings: b.effect.demand.clone(),
            basis: fixture::source("explicit-fixture-grant"),
        };
        let op = fixture::domain_operation(
            &probe,
            "grant-F",
            Command::SupervisedEffect(Box::new(EffectEvent::Grant(requested))),
        );
        let refused = probe.admit(op).unwrap_err();
        assert_eq!(format!("{refused:?}"), "Operational(Program(Transition))");
        assert_eq!(
            store.load(&key(), Access::ReadOnly).unwrap().revision(),
            revision
        );
    };
    assert_f_held();
    for (intent, label) in [(&b, "B"), (&e, "E")] {
        let component_root = Temp::new();
        let mut manager = LifecycleManager::new(&component_root, intent);
        manager.structured_stdout = Some(b"null\n");
        let result = execute_lifecycle(&store, &mut manager, &make_ids(intent, label));
        manager.inner.finish();
        assert!(result.diagnostic.is_none(), "{:?}", result.diagnostic);
        assert_eq!(manager.completions, 1);
        assert_eq!(manager.inner.created, 1);
        assert_f_held(); // Completion, parsed output and Report are not dependency acceptance.
    }
    let publish_edge = |edge: &str| {
        let mut current = store.load(&key(), Access::ReadOnly).unwrap();
        let revision = current.revision();
        fixture::release(&mut current, edge);
        for op in &current.operations()[revision as usize..] {
            store.admit(&key(), op.clone()).unwrap();
        }
    };
    publish_edge("BF");
    assert_f_held();
    publish_edge("EF");
    // F is prepared and granted only after both independently accepted inputs.
    // Its package/generation are captured fresh; no premature authority is saved.
    let mut prepared_f = store.load(&key(), Access::ReadOnly).unwrap();
    let revision = prepared_f.revision();
    let f = fixture::setup(&mut prepared_f, "F");
    for op in &prepared_f.operations()[revision as usize..] {
        store.admit(&key(), op.clone()).unwrap();
    }
    let current = store.load(&key(), Access::ReadOnly).unwrap();
    let revision = current.revision();
    let refused = store
        .admit(
            &key(),
            fixture::domain_operation(
                &current,
                "graph-reserve-F",
                Command::Domain(Box::new(DomainEvent::Reserve {
                    intent: f.clone(),
                    at: current.domains().unwrap().last_tick(),
                })),
            ),
        )
        .unwrap_err();
    // Accepted dependencies and fresh authority do not settle unknown cumulative
    // consumption. A larger fixture ceiling is not permission to erase debt.
    assert_eq!(format!("{:?}", refused.completion), "Rejected");
    assert_eq!(
        format!("{:?}", refused.error),
        "Domain(Domain(Debt(CpuExecutionMillis)))"
    );
    let final_kernel = store.load(&key(), Access::ReadOnly).unwrap();
    assert_eq!(final_kernel.revision(), revision);
    assert!(final_kernel.domains().unwrap().domain(&f.id).is_none());
    for intent in [&b, &e] {
        assert_eq!(count_event(&final_kernel, &intent.id, 0), 1);
        assert_eq!(count_event(&final_kernel, &intent.id, 2), 1);
        assert_eq!(count_event(&final_kernel, &intent.id, 3), 1);
        let record = final_kernel.domains().unwrap().domain(&intent.id).unwrap();
        assert_eq!(
            record.outcome,
            HistoricalOutcome::Reported(
                cortex_core::program::operational::EffectOutcome::Succeeded
            )
        );
        assert!(record.closure.is_none());
        assert!(record.holds(Resource::WorkerSlots));
        assert!(record.holds(Resource::DiskBytes));
        assert!(record.unresolved_consumption(Consumption::CpuExecutionMillis));
    }
    for kind in [0, 2, 3] {
        assert_eq!(count_event(&final_kernel, &f.id, kind), 0);
    }
}
