//! Commit-before-manager and commit-before-nonce boundaries. Recovery performs
//! observations only; it never resubmits creation or retransmits payload input.
use crate::{
    domain_bootstrap::{frame_read, frame_write, token},
    domain_manager::*,
};
use cortex_core::{
    program::{CoordinationGrant, OperationId, domain::*},
    temporal::SourceReference,
};
use cortex_evidence::program::{Command, CoordinationKernel, Operation};
use cortex_storage::{
    history_store::{Access, StoreId, StoreKey},
    program_store::{AdmissionReceipt, ProgramStore},
};
use serde_json::json;
use std::{
    io,
    os::unix::net::{UnixListener, UnixStream},
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, Instant},
};

/// Distinct canonical publication identities; none is a grant by itself.
#[derive(Clone)]
pub struct DomainOperations {
    /// Exact reserved domain.
    pub domain: DomainId,
    /// Commit-before-create identity.
    pub create: OperationId,
    /// Incarnation binding identity.
    pub bind: OperationId,
    /// Commit-before-release identity.
    pub release: OperationId,
    /// Captured nonce result identity.
    pub report: OperationId,
    /// Existing scoped coordinator authority assertion.
    pub grant: CoordinationGrant,
    /// Observation mechanism/version, not independent acceptance.
    pub basis: SourceReference,
    /// Logical tick at entry; elapsed monotonic time is added without resetting.
    pub at: u64,
}
/// Observations from one controlled execution. Durable uncertainty may outlive
/// the controller; a returned nonce is neither cleanup nor engineering acceptance.
pub struct DomainRun {
    /// Exact admitted creation receipt, when established.
    pub creation: Option<AdmissionReceipt>,
    /// Exact retained incarnation publication, when established.
    pub binding: Option<AdmissionReceipt>,
    /// Exact commit-before-payload receipt, when established.
    pub release: Option<AdmissionReceipt>,
    /// Captured fixed public nonce, absent if unresolved.
    pub nonce: Option<String>,
    /// Result observation receipt, independently from closure.
    pub report: Option<AdmissionReceipt>,
    /// Original redacted refusal/interruption.
    pub diagnostic: Option<DomainDiagnostic>,
    /// Existing uncertainty/release means this invocation performed no creation.
    pub recovery_only: bool,
}
impl DomainRun {
    fn empty() -> Self {
        Self {
            creation: None,
            binding: None,
            release: None,
            nonce: None,
            report: None,
            diagnostic: None,
            recovery_only: false,
        }
    }
}

/// Live read-only observation, not a closure/fencing receipt. In particular the
/// fixed service cannot prove snapshot exclusion, persistent scratch disposal
/// or the historical outcome. Missing manager state remains a diagnostic.
pub struct DomainRecoveryObservation {
    /// Exact currently inspected incarnation.
    pub identity: DomainIdentity,
    /// Kernel cgroup populated observation, not inferred from a parent exit.
    pub populated: bool,
    /// Sampled final/captured CPU counter, never a hard lifetime ceiling.
    pub cpu_micros: Option<u64>,
    /// Captured charged-memory peak when retained by the kernel.
    pub memory_peak: Option<u64>,
    /// Manager restart=no readback, not full fence proof.
    pub restart_prevented: bool,
    /// Broader incarnation resurrection exclusion is not proven by this adapter.
    pub snapshot_safe: bool,
}

pub(crate) struct DomainFinish<'a> {
    pub intent: &'a DomainIntent,
    pub binding: &'a DomainBindingObservation,
    pub release: &'a OperationId,
    pub value: &'a str,
    pub start: Instant,
    pub end: Instant,
    pub cancel: &'a AtomicBool,
}

/// The retained post-admission image immediately before one possible effect.
#[derive(Clone, Copy)]
pub(crate) enum TrialEffectCut<'a> {
    Create,
    Release(&'a DomainBindingObservation),
}

/// An additional trial requirement, composed with the existing runtime
/// authorization checks. Implementations must be bounded and must never wait
/// on a caller or reenter the store while the admission lock is held.
pub(crate) trait DomainTrialGuard {
    fn authorize(
        &self,
        kernel: &CoordinationKernel,
        intent: &DomainIntent,
        operation: &Operation,
        cut: TrialEffectCut<'_>,
        end: Instant,
    ) -> Result<(), DomainDiagnostic>;

    fn check(
        &self,
        kernel: &CoordinationKernel,
        intent: &DomainIntent,
        cut: TrialEffectCut<'_>,
        end: Instant,
    ) -> Result<(), DomainDiagnostic>;
}

fn trial_store_budget(
    store: &ProgramStore,
    trial: Option<&dyn DomainTrialGuard>,
    end: Instant,
) -> Result<(), DomainDiagnostic> {
    if trial.is_some()
        && end
            .checked_duration_since(Instant::now())
            .is_none_or(|remaining| remaining <= store.read_limits().timeout())
    {
        return Err(issue(
            DomainPhaseDiagnostic::Publication,
            DomainCause::Deadline,
        ));
    }
    Ok(())
}

pub(crate) trait DomainManager {
    fn check_context(
        &self,
        _kernel: &CoordinationKernel,
        _intent: &DomainIntent,
        _at: u64,
    ) -> Result<(), DomainDiagnostic> {
        Ok(())
    }
    fn setup(&mut self, _channel: &mut UnixStream, _end: Instant) -> Result<(), DomainDiagnostic> {
        Ok(())
    }
    fn finish(
        &mut self,
        channel: &mut UnixStream,
        context: DomainFinish<'_>,
        _authority: &mut dyn FnMut() -> Result<(), DomainDiagnostic>,
    ) -> Result<cortex_core::program::operational::EffectOutcome, DomainDiagnostic> {
        let reply = frame_read(channel, context.end)
            .map_err(|e| issue(DomainPhaseDiagnostic::Release, DomainCause::Io(e.kind())))?;
        if reply != json!([1, "nonce", context.intent.request.as_str(), context.value]) {
            return Err(issue(DomainPhaseDiagnostic::Release, DomainCause::Protocol));
        }
        Ok(cortex_core::program::operational::EffectOutcome::Succeeded)
    }
    fn is_nonce(&self) -> bool {
        true
    }
    fn isolation(&self) -> Option<&crate::native_linux::NativeIsolation> {
        None
    }
    fn verify_ready(&self, hello: &serde_json::Value) -> bool {
        hello.as_array().is_some_and(|a| a.len() == 6)
    }
    fn pin_process(
        &self,
        _actual: &ManagedObservation,
    ) -> Result<Option<crate::native_linux::StableProcess>, DomainDiagnostic> {
        Ok(None)
    }

    fn corresponds(&self, intent: &DomainIntent) -> Result<(), DomainDiagnostic>;
    fn create(&mut self, intent: &DomainIntent, end: Instant) -> Result<(), DomainDiagnostic>;
    fn observe(
        &mut self,
        intent: &DomainIntent,
        end: Instant,
    ) -> Result<ManagedObservation, DomainDiagnostic>;
    fn observe_tree(
        &mut self,
        _: &DomainIntent,
        _: Instant,
    ) -> Result<(ManagedObservation, Vec<DomainCgroupObservation>), DomainDiagnostic> {
        Err(issue(DomainPhaseDiagnostic::Closure, DomainCause::Contract))
    }
    fn stop(
        &mut self,
        intent: &DomainIntent,
        identity: &DomainIdentity,
        end: Instant,
    ) -> Result<(), DomainDiagnostic>;
}
impl DomainManager for SystemdNonceDomain {
    fn isolation(&self) -> Option<&crate::native_linux::NativeIsolation> {
        self.isolation()
    }
    fn verify_ready(&self, hello: &serde_json::Value) -> bool {
        self.verify_ready(hello)
    }
    fn pin_process(
        &self,
        actual: &ManagedObservation,
    ) -> Result<Option<crate::native_linux::StableProcess>, DomainDiagnostic> {
        crate::native_linux::StableProcess::acquire(actual.main_pid, actual.start_ticks)
            .map(Some)
            .map_err(|e| issue(DomainPhaseDiagnostic::Identity, DomainCause::Io(e.kind())))
    }

    fn corresponds(&self, i: &DomainIntent) -> Result<(), DomainDiagnostic> {
        self.corresponds(i)
    }
    fn create(&mut self, i: &DomainIntent, end: Instant) -> Result<(), DomainDiagnostic> {
        self.create(i, end)
    }
    fn observe(
        &mut self,
        i: &DomainIntent,
        end: Instant,
    ) -> Result<ManagedObservation, DomainDiagnostic> {
        self.observe(i, end)
    }
    fn observe_tree(
        &mut self,
        i: &DomainIntent,
        end: Instant,
    ) -> Result<(ManagedObservation, Vec<DomainCgroupObservation>), DomainDiagnostic> {
        self.observe_tree(i, end)
    }
    fn stop(
        &mut self,
        i: &DomainIntent,
        identity: &DomainIdentity,
        end: Instant,
    ) -> Result<(), DomainDiagnostic> {
        self.stop(i, identity, end)
    }
}

fn record(kernel: &CoordinationKernel, id: &DomainId) -> Result<DomainRecord, DomainDiagnostic> {
    kernel
        .domains()
        .and_then(|p| p.domain(id))
        .cloned()
        .ok_or_else(|| issue(DomainPhaseDiagnostic::Preparation, DomainCause::Contract))
}
fn operation(
    kernel: &CoordinationKernel,
    ids: &DomainOperations,
    id: OperationId,
    event: DomainEvent,
) -> Operation {
    Operation {
        id,
        expected: kernel.revision(),
        grant: ids.grant.clone(),
        command: Command::Domain(Box::new(event)),
    }
}
fn current(
    kernel: &CoordinationKernel,
    ids: &DomainOperations,
    start: Instant,
    end: Instant,
    cancel: &AtomicBool,
) -> Result<u64, DomainDiagnostic> {
    let elapsed = u64::try_from(start.elapsed().as_millis())
        .map_err(|_| issue(DomainPhaseDiagnostic::Release, DomainCause::Deadline))?;
    let at = ids
        .at
        .checked_add(elapsed)
        .ok_or_else(|| issue(DomainPhaseDiagnostic::Release, DomainCause::Deadline))?;
    if cancel.load(Ordering::SeqCst) || Instant::now() >= end {
        return Err(issue(
            DomainPhaseDiagnostic::Release,
            DomainCause::Authority,
        ));
    }
    kernel
        .check_domain_current(&ids.domain, at)
        .map_err(|_| issue(DomainPhaseDiagnostic::Release, DomainCause::Authority))?;
    if cancel.load(Ordering::SeqCst) || Instant::now() >= end {
        return Err(issue(
            DomainPhaseDiagnostic::Release,
            DomainCause::Authority,
        ));
    }
    Ok(at)
}

/// A controller-owned live pre-release gate. The channel and original monotonic
/// deadline cannot be cloned, deserialized or reconstructed from journal records.
/// Dropping this value only closes its descriptor: it never releases, stops,
/// settles, or changes the retained historical uncertainty.
#[must_use = "retain the gate until explicit release or cancellation; dropping it does not prove cleanup"]
pub struct PreparedNonceDomain {
    channel: UnixStream,
    process: Option<crate::native_linux::StableProcess>,
    native: Option<crate::native_linux::NativeReadiness>,
    store: StoreId,
    intent: DomainIntent,
    ids: DomainOperations,
    observation: DomainBindingObservation,
    creation: AdmissionReceipt,
    binding: AdmissionReceipt,
    nonce: String,
    start: Instant,
    end: Instant,
}
impl PreparedNonceDomain {
    /// Child-installed native controls, only for the selected isolated profile.
    /// These observations are not operational host qualification or permission.
    pub fn native_readiness(&self) -> Option<&crate::native_linux::NativeReadiness> {
        self.native.as_ref()
    }

    /// Exact retained domain, without exposing the channel or adding authority.
    pub fn domain(&self) -> &DomainId {
        &self.ids.domain
    }
    /// Durable observation corresponding to this channel, not live qualification.
    pub fn observation(&self) -> &DomainBindingObservation {
        &self.observation
    }
}
/// Preparation observations plus the optional uniquely owned pre-release channel.
/// Failure or recovery returns no channel and cannot repeat creation.
pub struct DomainPreparation {
    /// Durable receipts and bounded failure diagnostics; release/result stay absent.
    pub run: DomainRun,
    /// Present only after actual observation was durably bound.
    pub prepared: Option<PreparedNonceDomain>,
}

/// Prepare one fixed bootstrap and durably bind its channel without releasing it.
/// Actual-host invocation requires separate owner authority and qualification.
/// Reconstruction never creates a new handle, adopts a process or repeats create.
pub fn prepare_nonce_domain(
    store: &ProgramStore,
    key: &StoreKey,
    backend: &mut SystemdNonceDomain,
    ids: &DomainOperations,
    cancel: &AtomicBool,
) -> DomainPreparation {
    let start = Instant::now();
    let nonce = backend.selection.nonce.clone();
    let end = start.checked_add(Duration::from_millis(backend.selection.limits.wall_ms));
    let endpoint = store
        .load(key, Access::ReadOnly)
        .ok()
        .and_then(|k| record(&k, &ids.domain).ok())
        .and_then(|r| backend.endpoint(&r.intent.request).ok());
    match (end, endpoint) {
        (Some(end), Some(endpoint)) => prepare(
            store,
            key,
            backend,
            ExecutionRequest {
                ids,
                nonce: &nonce,
                cancel,
                start,
                end,
            },
            || {
                UnixListener::bind(endpoint)
                    .map_err(|e| issue(DomainPhaseDiagnostic::Creation, DomainCause::Io(e.kind())))
            },
        ),
        _ => {
            let mut run = DomainRun::empty();
            run.diagnostic = Some(issue(
                DomainPhaseDiagnostic::Preparation,
                DomainCause::Contract,
            ));
            DomainPreparation {
                run,
                prepared: None,
            }
        }
    }
}

/// Explicitly release the fixed nonce on a uniquely held preparation. Checks use
/// the original deadline, the current canonical authority and a fresh manager
/// observation inside the commit-before-release lock. The handle is consumed;
/// failure is not permission to reconstruct it or retransmit.
pub fn release_nonce_domain(
    store: &ProgramStore,
    key: &StoreKey,
    backend: &mut SystemdNonceDomain,
    prepared: PreparedNonceDomain,
    cancel: &AtomicBool,
) -> DomainRun {
    release(store, key, backend, prepared, cancel)
}

/// Convenience consumer of the same production preparation and explicit release
/// paths. It cannot bypass the retained-channel boundary or use another nonce.
pub fn execute_nonce_domain(
    store: &ProgramStore,
    key: &StoreKey,
    backend: &mut SystemdNonceDomain,
    ids: &DomainOperations,
    nonce: &str,
    cancel: &AtomicBool,
) -> DomainRun {
    if nonce != backend.selection.nonce {
        let mut run = DomainRun::empty();
        run.diagnostic = Some(issue(
            DomainPhaseDiagnostic::Preparation,
            DomainCause::Correspondence,
        ));
        return run;
    }
    let preparation = prepare_nonce_domain(store, key, backend, ids, cancel);
    match preparation.prepared {
        Some(prepared) => release_nonce_domain(store, key, backend, prepared, cancel),
        None => preparation.run,
    }
}

struct ExecutionRequest<'a> {
    ids: &'a DomainOperations,
    nonce: &'a str,
    cancel: &'a AtomicBool,
    start: Instant,
    end: Instant,
}

fn binding_observation(
    actual: &ManagedObservation,
    intent: &DomainIntent,
    basis: &SourceReference,
) -> Result<DomainBindingObservation, DomainDiagnostic> {
    let observation = DomainBindingObservation {
        identity: actual.identity.clone(),
        configuration: intent.configuration.clone(),
        main: DomainProcessObservation {
            pid: actual.main_pid,
            start_ticks: actual.start_ticks,
        },
        direct_members: actual.direct_members.clone(),
        controls: actual.controls.clone(),
        populated: actual.populated,
        basis: basis.clone(),
    };
    observation
        .validate()
        .map_err(|_| issue(DomainPhaseDiagnostic::Identity, DomainCause::Correspondence))?;
    Ok(observation)
}

fn prepare<M: DomainManager>(
    store: &ProgramStore,
    key: &StoreKey,
    manager: &mut M,
    request: ExecutionRequest<'_>,
    listener: impl FnOnce() -> Result<UnixListener, DomainDiagnostic>,
) -> DomainPreparation {
    prepare_with_trial(store, key, manager, request, listener, None)
}

fn prepare_with_trial<M: DomainManager>(
    store: &ProgramStore,
    key: &StoreKey,
    manager: &mut M,
    request: ExecutionRequest<'_>,
    listener: impl FnOnce() -> Result<UnixListener, DomainDiagnostic>,
    trial: Option<&dyn DomainTrialGuard>,
) -> DomainPreparation {
    let ExecutionRequest {
        ids,
        nonce,
        cancel,
        start,
        end,
    } = request;
    let mut run = DomainRun::empty();
    let attempt = || -> Result<Option<PreparedNonceDomain>, DomainDiagnostic> {
        trial_store_budget(store, trial, end)?;
        if !token(nonce) {
            return Err(issue(
                DomainPhaseDiagnostic::Preparation,
                DomainCause::Contract,
            ));
        }
        let kernel = store
            .load(key, Access::ReadOnly)
            .map_err(|_| issue(DomainPhaseDiagnostic::Preparation, DomainCause::Journal))?;
        let domain = record(&kernel, &ids.domain)?;
        if domain.phase != DomainPhase::Reserved {
            run.recovery_only = true;
            return Ok(None);
        }
        manager.corresponds(&domain.intent)?;
        let at = current(&kernel, ids, start, end, cancel)?;
        manager.check_context(&kernel, &domain.intent, at)?;
        let create = operation(
            &kernel,
            ids,
            ids.create.clone(),
            DomainEvent::CreateRequested {
                domain: ids.domain.clone(),
                at,
            },
        );
        if let Some(trial) = trial {
            trial.authorize(
                &kernel,
                &domain.intent,
                &create,
                TrialEffectCut::Create,
                end,
            )?;
        }
        trial_store_budget(store, trial, end)?;
        let (creation, created) = store
            .dispatch_domain(key, create, |k| {
                let at = current(k, ids, start, end, cancel)?;
                manager.check_context(k, &domain.intent, at)?;
                if let Some(trial) = trial {
                    trial.check(k, &domain.intent, TrialEffectCut::Create, end)?;
                }
                let listener = listener()?;
                listener.set_nonblocking(true).map_err(|e| {
                    issue(DomainPhaseDiagnostic::Creation, DomainCause::Io(e.kind()))
                })?;
                manager.create(&domain.intent, end)?;
                Ok::<_, DomainDiagnostic>(listener)
            })
            .map_err(|_| issue(DomainPhaseDiagnostic::Publication, DomainCause::Journal))?;
        run.creation = Some(creation);
        let Some(created) = created else {
            run.recovery_only = true;
            return Ok(None);
        };
        let listener = created?;
        let mut channel = loop {
            trial_store_budget(store, trial, end)?;
            current(
                &store
                    .load(key, Access::ReadOnly)
                    .map_err(|_| issue(DomainPhaseDiagnostic::Identity, DomainCause::Journal))?,
                ids,
                start,
                end,
                cancel,
            )?;
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(1))
                }
                Err(e) => {
                    return Err(issue(
                        DomainPhaseDiagnostic::Identity,
                        DomainCause::Io(e.kind()),
                    ));
                }
            }
        };
        let peer = rustix::net::sockopt::socket_peercred(&channel).map_err(|e| {
            issue(
                DomainPhaseDiagnostic::Identity,
                DomainCause::Io(io::Error::from(e).kind()),
            )
        })?;
        manager.setup(&mut channel, end)?;
        let hello = frame_read(&mut channel, end)
            .map_err(|e| issue(DomainPhaseDiagnostic::Identity, DomainCause::Io(e.kind())))?;
        let actual = manager.observe(&domain.intent, end)?;
        if peer.pid.as_raw_pid() as u32 != actual.main_pid
            || !actual.populated
            || !manager.verify_ready(&hello)
            || hello[0] != 1
            || hello[1] != "hello"
            || hello[2] != domain.intent.request.as_str()
            || hello[3] != actual.main_pid
            || hello[4] != actual.start_ticks
            || hello[5] != actual.identity.boot
        {
            return Err(issue(
                DomainPhaseDiagnostic::Identity,
                DomainCause::Correspondence,
            ));
        }
        let process = manager.pin_process(&actual)?;
        let native = manager.isolation().map(|policy| {
            crate::native_linux::NativeReadiness::observed(policy, &actual.identity.boot)
        });
        let observation = binding_observation(&actual, &domain.intent, &ids.basis)?;
        trial_store_budget(store, trial, end)?;
        let kernel = store
            .load(key, Access::ReadOnly)
            .map_err(|_| issue(DomainPhaseDiagnostic::Publication, DomainCause::Journal))?;
        let at = current(&kernel, ids, start, end, cancel)?;
        let bound = operation(
            &kernel,
            ids,
            ids.bind.clone(),
            DomainEvent::BoundObserved {
                domain: ids.domain.clone(),
                observation: observation.clone(),
                at,
            },
        );
        trial_store_budget(store, trial, end)?;
        let binding = store
            .admit(key, bound)
            .map_err(|_| issue(DomainPhaseDiagnostic::Publication, DomainCause::Journal))?;
        run.binding = Some(binding);
        Ok(Some(PreparedNonceDomain {
            channel,
            process,
            native,
            store: store.id(),
            intent: domain.intent,
            ids: ids.clone(),
            observation,
            creation,
            binding,
            nonce: nonce.into(),
            start,
            end,
        }))
    };
    let prepared = match attempt() {
        Ok(handle) => handle,
        Err(error) => {
            run.diagnostic = Some(error);
            None
        }
    };
    DomainPreparation { run, prepared }
}

fn release<M: DomainManager>(
    store: &ProgramStore,
    key: &StoreKey,
    manager: &mut M,
    prepared: PreparedNonceDomain,
    cancel: &AtomicBool,
) -> DomainRun {
    release_with_trial(store, key, manager, prepared, cancel, None)
}

fn release_with_trial<M: DomainManager>(
    store: &ProgramStore,
    key: &StoreKey,
    manager: &mut M,
    mut prepared: PreparedNonceDomain,
    cancel: &AtomicBool,
    trial: Option<&dyn DomainTrialGuard>,
) -> DomainRun {
    let mut run = DomainRun::empty();
    run.creation = Some(prepared.creation);
    run.binding = Some(prepared.binding);
    let mut attempt = || -> Result<(), DomainDiagnostic> {
        if store.id() != prepared.store {
            return Err(issue(
                DomainPhaseDiagnostic::Release,
                DomainCause::Correspondence,
            ));
        }
        let ids = &prepared.ids;
        trial_store_budget(store, trial, prepared.end)?;
        let kernel = store
            .load(key, Access::ReadOnly)
            .map_err(|_| issue(DomainPhaseDiagnostic::Publication, DomainCause::Journal))?;
        let at = current(&kernel, ids, prepared.start, prepared.end, cancel)?;
        let domain = record(&kernel, &ids.domain)?;
        if domain.intent != prepared.intent
            || domain.binding_observation.as_ref() != Some(&prepared.observation)
            || domain.phase != DomainPhase::Bound
        {
            return Err(issue(
                DomainPhaseDiagnostic::Release,
                DomainCause::Correspondence,
            ));
        }
        manager.corresponds(&prepared.intent)?;
        let observed = manager.observe(&prepared.intent, prepared.end)?;
        if binding_observation(&observed, &prepared.intent, &ids.basis)? != prepared.observation {
            return Err(issue(
                DomainPhaseDiagnostic::Release,
                DomainCause::Correspondence,
            ));
        }
        current(&kernel, ids, prepared.start, prepared.end, cancel)?;
        let release = operation(
            &kernel,
            ids,
            ids.release.clone(),
            DomainEvent::ReleaseRequested {
                domain: ids.domain.clone(),
                identity: prepared.observation.identity.clone(),
                at,
            },
        );
        if let Some(trial) = trial {
            trial.authorize(
                &kernel,
                &prepared.intent,
                &release,
                TrialEffectCut::Release(&prepared.observation),
                prepared.end,
            )?;
        }
        trial_store_budget(store, trial, prepared.end)?;
        let (receipt, released) = store
            .dispatch_domain(key, release, |k| {
                let at = current(k, ids, prepared.start, prepared.end, cancel)?;
                manager.check_context(k, &prepared.intent, at)?;
                let current_record = record(k, &ids.domain)?;
                if current_record.intent != prepared.intent
                    || current_record.binding_observation.as_ref() != Some(&prepared.observation)
                {
                    return Err(issue(
                        DomainPhaseDiagnostic::Release,
                        DomainCause::Correspondence,
                    ));
                }
                // Reobserve after commit under the journal lifecycle lock, not before
                // waiting for that lock. Revocation cannot interleave publication.
                manager.corresponds(&prepared.intent)?;
                let observed = manager.observe(&prepared.intent, prepared.end)?;
                if binding_observation(&observed, &prepared.intent, &ids.basis)?
                    != prepared.observation
                {
                    return Err(issue(
                        DomainPhaseDiagnostic::Release,
                        DomainCause::Correspondence,
                    ));
                }
                current(k, ids, prepared.start, prepared.end, cancel)?;
                if let Some(native) = &prepared.native {
                    native.check_context().map_err(|e| {
                        issue(DomainPhaseDiagnostic::Release, DomainCause::Io(e.kind()))
                    })?;
                }
                if let Some(process) = &prepared.process {
                    process.require_live().map_err(|e| {
                        issue(DomainPhaseDiagnostic::Release, DomainCause::Io(e.kind()))
                    })?;
                }
                if let Some(trial) = trial {
                    trial.check(
                        k,
                        &prepared.intent,
                        TrialEffectCut::Release(&prepared.observation),
                        prepared.end,
                    )?;
                }
                frame_write(
                    &mut prepared.channel,
                    &json!([
                        1,
                        "release",
                        prepared.intent.request.as_str(),
                        prepared.nonce
                    ]),
                    prepared.end,
                )
                .map_err(|e| issue(DomainPhaseDiagnostic::Release, DomainCause::Io(e.kind())))
            })
            .map_err(|_| issue(DomainPhaseDiagnostic::Publication, DomainCause::Journal))?;
        run.release = Some(receipt);
        let Some(released) = released else {
            run.recovery_only = true;
            return Ok(());
        };
        released?;
        let mut next_check = Instant::now();
        let mut authority = || {
            if Instant::now() >= next_check {
                trial_store_budget(store, trial, prepared.end)?;
                let fresh = store
                    .load(key, Access::ReadOnly)
                    .map_err(|_| issue(DomainPhaseDiagnostic::Release, DomainCause::Journal))?;
                current(&fresh, ids, prepared.start, prepared.end, cancel)?;
                next_check = Instant::now() + Duration::from_millis(1);
            }
            Ok(())
        };
        let outcome = manager.finish(
            &mut prepared.channel,
            DomainFinish {
                intent: &prepared.intent,
                binding: &prepared.observation,
                release: &ids.release,
                value: &prepared.nonce,
                start: prepared.start,
                end: prepared.end,
                cancel,
            },
            &mut authority,
        )?;
        if manager.is_nonce() {
            run.nonce = Some(prepared.nonce.clone());
        }
        trial_store_budget(store, trial, prepared.end)?;
        let kernel = store
            .load(key, Access::ReadOnly)
            .map_err(|_| issue(DomainPhaseDiagnostic::Publication, DomainCause::Journal))?;
        let at = ids
            .at
            .checked_add(
                u64::try_from(prepared.start.elapsed().as_millis()).map_err(|_| {
                    issue(DomainPhaseDiagnostic::Publication, DomainCause::Deadline)
                })?,
            )
            .ok_or_else(|| issue(DomainPhaseDiagnostic::Publication, DomainCause::Deadline))?;
        let report = operation(
            &kernel,
            ids,
            ids.report.clone(),
            DomainEvent::Report {
                domain: ids.domain.clone(),
                identity: prepared.observation.identity.clone(),
                outcome,
                at,
                basis: ids.basis.clone(),
            },
        );
        trial_store_budget(store, trial, prepared.end)?;
        run.report = Some(
            store
                .admit(key, report)
                .map_err(|_| issue(DomainPhaseDiagnostic::Publication, DomainCause::Journal))?,
        );
        Ok(())
    };
    if let Err(error) = attempt() {
        run.diagnostic = Some(error);
    }
    run
}

#[cfg(test)]
fn execute<M: DomainManager>(
    store: &ProgramStore,
    key: &StoreKey,
    manager: &mut M,
    request: ExecutionRequest<'_>,
    listener: impl FnOnce() -> Result<UnixListener, DomainDiagnostic>,
) -> DomainRun {
    let cancel = request.cancel;
    let preparation = prepare(store, key, manager, request, listener);
    match preparation.prepared {
        Some(prepared) => release(store, key, manager, prepared, cancel),
        None => preparation.run,
    }
}

/// Reinspect only the exact retained request/incarnation. This never creates a
/// domain, replays payload, adopts a process or settles resources. A missing or
/// replaced manager/unit/cgroup remains an error rather than evidence of absence.
pub fn inspect_nonce_domain(
    store: &ProgramStore,
    key: &StoreKey,
    backend: &mut SystemdNonceDomain,
    domain: &DomainId,
) -> Result<DomainRecoveryObservation, DomainDiagnostic> {
    let kernel = store
        .load(key, Access::ReadOnly)
        .map_err(|_| issue(DomainPhaseDiagnostic::Identity, DomainCause::Journal))?;
    let record = record(&kernel, domain)?;
    if matches!(
        record.phase,
        DomainPhase::Reserved | DomainPhase::CancelledBeforeCreate
    ) {
        return Err(issue(
            DomainPhaseDiagnostic::Identity,
            DomainCause::Contract,
        ));
    }
    let observation = backend.observe(&record.intent, Instant::now() + Duration::from_secs(1))?;
    if record
        .identity
        .as_ref()
        .is_some_and(|id| *id != observation.identity)
    {
        return Err(issue(
            DomainPhaseDiagnostic::Identity,
            DomainCause::Correspondence,
        ));
    }
    Ok(DomainRecoveryObservation {
        identity: observation.identity,
        populated: observation.populated,
        cpu_micros: observation.cpu_micros,
        memory_peak: observation.memory_peak,
        restart_prevented: observation.restart_prevented,
        snapshot_safe: false,
    })
}

/// Capture and durably retain a bounded read-only post-cancellation sample of
/// the exact bound incarnation. No create, release, stop, closure or settlement
/// is performed. An absent/replaced identity is an error, never zero usage.
/// Direct-member readbacks leave whole-domain/helper/stream/snapshot proof held.
pub fn observe_nonce_domain_closure(
    store: &ProgramStore,
    key: &StoreKey,
    backend: &mut SystemdNonceDomain,
    ids: &DomainOperations,
    observation_id: OperationId,
) -> Result<AdmissionReceipt, DomainDiagnostic> {
    observe_closure(store, key, backend, ids, observation_id)
}
fn observe_closure<M: DomainManager>(
    store: &ProgramStore,
    key: &StoreKey,
    backend: &mut M,
    ids: &DomainOperations,
    observation_id: OperationId,
) -> Result<AdmissionReceipt, DomainDiagnostic> {
    let start = Instant::now();
    let end = start + Duration::from_secs(1);
    let kernel = store
        .load(key, Access::ReadOnly)
        .map_err(|_| issue(DomainPhaseDiagnostic::Closure, DomainCause::Journal))?;
    let domain = record(&kernel, &ids.domain)?;
    let binding = domain
        .binding_observation
        .as_ref()
        .ok_or_else(|| issue(DomainPhaseDiagnostic::Closure, DomainCause::Correspondence))?;
    if !domain.cancelled || domain.closure.is_some() {
        return Err(issue(DomainPhaseDiagnostic::Closure, DomainCause::Contract));
    }
    backend.corresponds(&domain.intent)?;
    let actual = backend.observe(&domain.intent, end)?;
    if Instant::now() >= end || actual.identity != binding.identity {
        return Err(issue(
            DomainPhaseDiagnostic::Closure,
            DomainCause::Correspondence,
        ));
    }
    let at = kernel
        .domains()
        .ok_or_else(|| issue(DomainPhaseDiagnostic::Closure, DomainCause::Contract))?
        .last_tick()
        .checked_add(
            u64::try_from(start.elapsed().as_millis())
                .map_err(|_| issue(DomainPhaseDiagnostic::Closure, DomainCause::Deadline))?,
        )
        .ok_or_else(|| issue(DomainPhaseDiagnostic::Closure, DomainCause::Deadline))?;
    let observed_main = match (actual.main_pid, actual.start_ticks) {
        (0, 0) => None,
        (pid, start_ticks) if pid != 0 && start_ticks != 0 => {
            Some(DomainProcessObservation { pid, start_ticks })
        }
        _ => {
            return Err(issue(
                DomainPhaseDiagnostic::Closure,
                DomainCause::Correspondence,
            ));
        }
    };
    let observation = DomainClosureObservation {
        identity: actual.identity,
        configuration: domain.intent.configuration.clone(),
        scope: domain.intent.effect.scope.clone(),
        binding_main: binding.main.clone(),
        observed_main,
        direct_members: actual.direct_members,
        populated: actual.populated,
        restart_prevented: actual.restart_prevented,
        cpu_micros: actual.cpu_micros,
        memory_peak: actual.memory_peak,
        observed_at: at,
        basis: ids.basis.clone(),
    };
    observation
        .validate()
        .map_err(|_| issue(DomainPhaseDiagnostic::Closure, DomainCause::Correspondence))?;
    // Use the revision sampled before manager I/O. A concurrent writer makes
    // publication stale; never retimestamp, repeat manager effects or overwrite.
    store
        .admit(
            key,
            operation(
                &kernel,
                ids,
                observation_id,
                DomainEvent::ClosureObserved {
                    domain: ids.domain.clone(),
                    observation,
                    at,
                },
            ),
        )
        .map_err(|_| issue(DomainPhaseDiagnostic::Publication, DomainCause::Journal))
}

/// Capture a bounded recursive native tree after durable cancellation. Matched
/// traversals are non-atomic facts; helpers outside the tree, retained streams,
/// snapshot exclusion and final consumption remain unproven. This publishes an
/// observation and returns its journal admission receipt; it performs no host
/// mutation, closure, settlement, adoption or retry.
pub fn observe_nonce_domain_closure_tree(
    store: &ProgramStore,
    key: &StoreKey,
    backend: &mut SystemdNonceDomain,
    ids: &DomainOperations,
    observation_id: OperationId,
) -> Result<AdmissionReceipt, DomainDiagnostic> {
    observe_closure_tree(store, key, backend, ids, observation_id)
}
fn observe_closure_tree<M: DomainManager>(
    store: &ProgramStore,
    key: &StoreKey,
    backend: &mut M,
    ids: &DomainOperations,
    observation_id: OperationId,
) -> Result<AdmissionReceipt, DomainDiagnostic> {
    let start = Instant::now();
    let operation = capture_closure_tree(
        store,
        key,
        backend,
        ids,
        observation_id,
        start,
        start + Duration::from_secs(1),
    )?;
    store
        .admit(key, operation)
        .map_err(|_| issue(DomainPhaseDiagnostic::Publication, DomainCause::Journal))
}

/// Failure of a managed tree observation or of its distinct journal publication.
#[derive(Debug)]
pub enum ManagedClosureObservationError {
    /// Capture refused without submitting a journal entry.
    Capture(DomainDiagnostic),
    /// Canonical admission failed. Preserve its original completion classification.
    Admission(cortex_storage::program_store::AdmissionFailure),
}

/// Capture and publish a partial post-cancel observation of a still-held managed
/// executable. Uses the original deadline and the existing locked admission
/// rules. After indeterminate publication, inspect the journal for the exact
/// observation ID. This function grants no authority to repeat effects.
pub fn observe_managed_executable_closure_tree(
    store: &ProgramStore,
    key: &StoreKey,
    backend: &mut crate::native_execution::managed_backend::ManagedExecutionDomain,
    ids: &DomainOperations,
    observation_id: OperationId,
) -> Result<AdmissionReceipt, ManagedClosureObservationError> {
    backend
        .check_bound_store(store.id())
        .map_err(ManagedClosureObservationError::Capture)?;
    let start = Instant::now();
    let end = backend.observation_end().ok_or_else(|| {
        ManagedClosureObservationError::Capture(issue(
            DomainPhaseDiagnostic::Closure,
            DomainCause::Correspondence,
        ))
    })?;
    let operation = capture_closure_tree(store, key, backend, ids, observation_id, start, end)
        .map_err(ManagedClosureObservationError::Capture)?;
    store
        .admit_observed_domain_until(key, operation, end, || Ok(()))
        .map_err(ManagedClosureObservationError::Admission)
}

fn capture_closure_tree<M: DomainManager>(
    store: &ProgramStore,
    key: &StoreKey,
    backend: &mut M,
    ids: &DomainOperations,
    observation_id: OperationId,
    start: Instant,
    end: Instant,
) -> Result<Operation, DomainDiagnostic> {
    if start >= end {
        return Err(issue(DomainPhaseDiagnostic::Closure, DomainCause::Deadline));
    }
    let kernel = store
        .load_read_only_until(key, end)
        .map_err(|_| issue(DomainPhaseDiagnostic::Closure, DomainCause::Journal))?;
    let domain = record(&kernel, &ids.domain)?;
    let binding = domain
        .binding_observation
        .as_ref()
        .ok_or_else(|| issue(DomainPhaseDiagnostic::Closure, DomainCause::Correspondence))?;
    if !domain.cancelled || domain.closure.is_some() {
        return Err(issue(DomainPhaseDiagnostic::Closure, DomainCause::Contract));
    }
    backend.corresponds(&domain.intent)?;
    let (actual, nodes) = backend.observe_tree(&domain.intent, end)?;
    if Instant::now() >= end || actual.identity != binding.identity {
        return Err(issue(
            DomainPhaseDiagnostic::Closure,
            DomainCause::Correspondence,
        ));
    }
    let at = kernel
        .domains()
        .ok_or_else(|| issue(DomainPhaseDiagnostic::Closure, DomainCause::Contract))?
        .last_tick()
        .checked_add(
            u64::try_from(start.elapsed().as_millis())
                .map_err(|_| issue(DomainPhaseDiagnostic::Closure, DomainCause::Deadline))?,
        )
        .ok_or_else(|| issue(DomainPhaseDiagnostic::Closure, DomainCause::Deadline))?;
    let observed_main = match (actual.main_pid, actual.start_ticks) {
        (0, 0) => None,
        (pid, start_ticks) if pid != 0 && start_ticks != 0 => {
            Some(DomainProcessObservation { pid, start_ticks })
        }
        _ => {
            return Err(issue(
                DomainPhaseDiagnostic::Closure,
                DomainCause::Correspondence,
            ));
        }
    };
    let observation = DomainClosureTreeObservation {
        sample: DomainClosureObservation {
            identity: actual.identity,
            configuration: domain.intent.configuration.clone(),
            scope: domain.intent.effect.scope.clone(),
            binding_main: binding.main.clone(),
            observed_main,
            direct_members: actual.direct_members,
            populated: actual.populated,
            restart_prevented: actual.restart_prevented,
            cpu_micros: actual.cpu_micros,
            memory_peak: actual.memory_peak,
            observed_at: at,
            basis: ids.basis.clone(),
        },
        nodes,
    };
    observation
        .validate()
        .map_err(|_| issue(DomainPhaseDiagnostic::Closure, DomainCause::Correspondence))?;
    if Instant::now() >= end {
        return Err(issue(DomainPhaseDiagnostic::Closure, DomainCause::Deadline));
    }
    Ok(operation(
        &kernel,
        ids,
        observation_id,
        DomainEvent::ClosureTreeObserved {
            domain: ids.domain.clone(),
            observation,
            at,
        },
    ))
}

/// Consume a retained pre-release channel through canonical cancellation. The
/// channel stays owned until cancellation/stop returns, then closes even on error.
/// Neither descriptor closure nor a stop acknowledgement proves domain closure.
pub fn cancel_prepared_nonce_domain(
    store: &ProgramStore,
    key: &StoreKey,
    backend: &mut SystemdNonceDomain,
    prepared: PreparedNonceDomain,
    operation: Operation,
) -> Result<AdmissionReceipt, DomainDiagnostic> {
    cancel_prepared(store, key, backend, prepared, operation)
}
fn cancel_prepared<M: DomainManager>(
    store: &ProgramStore,
    key: &StoreKey,
    manager: &mut M,
    prepared: PreparedNonceDomain,
    operation: Operation,
) -> Result<AdmissionReceipt, DomainDiagnostic> {
    let Command::Domain(event) = &operation.command else {
        return Err(issue(DomainPhaseDiagnostic::Closure, DomainCause::Contract));
    };
    if store.id() != prepared.store
        || !matches!(event.as_ref(), DomainEvent::Cancel { domain, .. } if domain == &prepared.ids.domain)
    {
        return Err(issue(
            DomainPhaseDiagnostic::Closure,
            DomainCause::Correspondence,
        ));
    }
    let kernel = store
        .load(key, Access::ReadOnly)
        .map_err(|_| issue(DomainPhaseDiagnostic::Closure, DomainCause::Journal))?;
    let domain = record(&kernel, &prepared.ids.domain)?;
    if domain.intent != prepared.intent
        || domain.binding_observation.as_ref() != Some(&prepared.observation)
    {
        return Err(issue(
            DomainPhaseDiagnostic::Closure,
            DomainCause::Correspondence,
        ));
    }
    let result = cancel_domain(store, key, manager, operation);
    drop(prepared); // Only this owned channel; retained uncertainty is unchanged.
    result
}

/// Persist one cancellation request before targeting the exact still-matching
/// manager incarnation. A stop request alone never records closure or a refund.
pub fn cancel_nonce_domain(
    store: &ProgramStore,
    key: &StoreKey,
    backend: &mut SystemdNonceDomain,
    operation: Operation,
) -> Result<AdmissionReceipt, DomainDiagnostic> {
    cancel_domain(store, key, backend, operation)
}
fn cancel_domain<M: DomainManager>(
    store: &ProgramStore,
    key: &StoreKey,
    backend: &mut M,
    operation: Operation,
) -> Result<AdmissionReceipt, DomainDiagnostic> {
    let Command::Domain(event) = &operation.command else {
        return Err(issue(DomainPhaseDiagnostic::Closure, DomainCause::Contract));
    };
    let DomainEvent::Cancel { domain, .. } = event.as_ref() else {
        return Err(issue(DomainPhaseDiagnostic::Closure, DomainCause::Contract));
    };
    let id = domain.clone();
    let (receipt, result) = store
        .dispatch_domain(key, operation, |kernel| {
            let record = record(kernel, &id)?;
            if record.phase == DomainPhase::CancelledBeforeCreate {
                return Ok(());
            }
            let identity = record.identity.ok_or_else(|| {
                issue(DomainPhaseDiagnostic::Closure, DomainCause::Correspondence)
            })?;
            backend.stop(
                &record.intent,
                &identity,
                Instant::now() + Duration::from_millis(1000),
            )
        })
        .map_err(|_| issue(DomainPhaseDiagnostic::Publication, DomainCause::Journal))?;
    if let Some(result) = result {
        result?;
    }
    Ok(receipt)
}

#[cfg(test)]
pub(crate) mod tests;

/// Strict trusted-executable observations plus the original canonical receipts.
/// Neither direct exit nor captured output establishes whole-domain closure.
pub struct ExecutableDomainRun {
    /// Existing durable domain sequence.
    pub domain: DomainRun,
    /// Bounded actual streams and artifact observations, if collection completed.
    pub execution: Option<crate::native_execution::backend::ExecutionObservation>,
}
/// Prepare a trusted executable using exactly the existing domain admission path.
/// No delegation means refusal before bootstrap creation; there is no fallback.
pub fn prepare_executable_domain(
    store: &ProgramStore,
    key: &StoreKey,
    backend: &mut crate::native_execution::backend::NativeExecutionDomain,
    ids: &DomainOperations,
    cancel: &AtomicBool,
) -> DomainPreparation {
    let start = Instant::now();
    let end = start.checked_add(Duration::from_millis(backend.wall_ms()));
    let value = backend.digest().to_owned();
    let endpoint = backend.endpoint();
    match end {
        Some(end) => prepare(
            store,
            key,
            backend,
            ExecutionRequest {
                ids,
                nonce: &value,
                cancel,
                start,
                end,
            },
            || {
                UnixListener::bind(endpoint)
                    .map_err(|e| issue(DomainPhaseDiagnostic::Creation, DomainCause::Io(e.kind())))
            },
        ),
        None => {
            let mut run = DomainRun::empty();
            run.diagnostic = Some(issue(
                DomainPhaseDiagnostic::Preparation,
                DomainCause::Deadline,
            ));
            DomainPreparation {
                run,
                prepared: None,
            }
        }
    }
}
/// Consume the uniquely prepared gate once. Collection runs outside the journal
/// lock; missing acknowledgments cannot reconstruct or replay the release.
pub fn release_executable_domain(
    store: &ProgramStore,
    key: &StoreKey,
    backend: &mut crate::native_execution::backend::NativeExecutionDomain,
    prepared: PreparedNonceDomain,
    cancel: &AtomicBool,
) -> ExecutableDomainRun {
    let domain = release(store, key, backend, prepared, cancel);
    ExecutableDomainRun {
        domain,
        execution: backend.take_observation(),
    }
}

/// Persist cancellation before requesting termination of the exact native group.
/// This observes no closure and never authorizes settlement, replay or refund.
pub fn cancel_executable_domain(
    store: &ProgramStore,
    key: &StoreKey,
    backend: &mut crate::native_execution::backend::NativeExecutionDomain,
    operation: Operation,
) -> Result<AdmissionReceipt, DomainDiagnostic> {
    cancel_domain(store, key, backend, operation)
}

/// Prepare the explicitly selected independent-manager executable profile.
/// Missing delegation or manager facts cannot fall back to direct execution.
pub fn prepare_managed_executable_domain(
    store: &ProgramStore,
    key: &StoreKey,
    backend: &mut crate::native_execution::managed_backend::ManagedExecutionDomain,
    ids: &DomainOperations,
    cancel: &AtomicBool,
) -> DomainPreparation {
    prepare_managed_executable_domain_until(store, key, backend, ids, cancel, None, None)
}

pub(crate) fn prepare_managed_executable_domain_until(
    store: &ProgramStore,
    key: &StoreKey,
    backend: &mut crate::native_execution::managed_backend::ManagedExecutionDomain,
    ids: &DomainOperations,
    cancel: &AtomicBool,
    limit: Option<Instant>,
    trial: Option<&dyn DomainTrialGuard>,
) -> DomainPreparation {
    let start = Instant::now();
    if let Err(error) = backend.bind_cpu_store(store.id(), start) {
        let mut run = DomainRun::empty();
        run.diagnostic = Some(error);
        return DomainPreparation {
            run,
            prepared: None,
        };
    }
    let end = start
        .checked_add(Duration::from_millis(backend.wall_ms()))
        .map(|end| limit.map_or(end, |limit| end.min(limit)))
        .filter(|end| *end > start && trial_store_budget(store, trial, *end).is_ok());
    let value = backend.digest().to_owned();
    let endpoint = end.and_then(|_| {
        store
            .load(key, Access::ReadOnly)
            .ok()
            .and_then(|k| record(&k, &ids.domain).ok())
            .and_then(|r| backend.endpoint(&r.intent.request).ok())
    });
    match (end, endpoint) {
        (Some(end), Some(endpoint)) => prepare_with_trial(
            store,
            key,
            backend,
            ExecutionRequest {
                ids,
                nonce: &value,
                cancel,
                start,
                end,
            },
            || {
                UnixListener::bind(endpoint)
                    .map_err(|e| issue(DomainPhaseDiagnostic::Creation, DomainCause::Io(e.kind())))
            },
            trial,
        ),
        _ => {
            let mut run = DomainRun::empty();
            run.diagnostic = Some(issue(
                DomainPhaseDiagnostic::Preparation,
                DomainCause::Deadline,
            ));
            DomainPreparation {
                run,
                prepared: None,
            }
        }
    }
}
/// Consume the original unique managed gate through canonical locked release.
pub fn release_managed_executable_domain(
    store: &ProgramStore,
    key: &StoreKey,
    backend: &mut crate::native_execution::managed_backend::ManagedExecutionDomain,
    prepared: PreparedNonceDomain,
    cancel: &AtomicBool,
) -> ExecutableDomainRun {
    release_managed_executable_domain_with_trial(store, key, backend, prepared, cancel, None)
}

pub(crate) fn release_managed_executable_domain_with_trial(
    store: &ProgramStore,
    key: &StoreKey,
    backend: &mut crate::native_execution::managed_backend::ManagedExecutionDomain,
    prepared: PreparedNonceDomain,
    cancel: &AtomicBool,
    trial: Option<&dyn DomainTrialGuard>,
) -> ExecutableDomainRun {
    let domain = release_with_trial(store, key, backend, prepared, cancel, trial);
    ExecutableDomainRun {
        domain,
        execution: backend.take_observation(),
    }
}
/// Commit cancellation before the exact same-incarnation manager stop request.
/// A stop acknowledgment is not closure, settlement or reusable capacity.
pub fn cancel_managed_executable_domain(
    store: &ProgramStore,
    key: &StoreKey,
    backend: &mut crate::native_execution::managed_backend::ManagedExecutionDomain,
    operation: Operation,
) -> Result<AdmissionReceipt, DomainDiagnostic> {
    backend.check_bound_store(store.id())?;
    cancel_domain(store, key, backend, operation)
}
