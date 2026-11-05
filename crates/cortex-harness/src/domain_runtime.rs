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
    history_store::{Access, StoreKey},
    program_store::{ProgramStore, Receipt},
};
use serde_json::json;
use std::{
    io,
    os::unix::net::UnixListener,
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
    pub creation: Option<Receipt>,
    /// Exact retained incarnation publication, when established.
    pub binding: Option<Receipt>,
    /// Exact commit-before-payload receipt, when established.
    pub release: Option<Receipt>,
    /// Captured fixed public nonce, absent if unresolved.
    pub nonce: Option<String>,
    /// Result observation receipt, independently from closure.
    pub report: Option<Receipt>,
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

trait DomainManager {
    fn corresponds(&self, intent: &DomainIntent) -> Result<(), DomainDiagnostic>;
    fn create(&mut self, intent: &DomainIntent, end: Instant) -> Result<(), DomainDiagnostic>;
    fn observe(
        &mut self,
        intent: &DomainIntent,
        end: Instant,
    ) -> Result<ManagedObservation, DomainDiagnostic>;
}
impl DomainManager for SystemdNonceDomain {
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

/// Execute the one fixed public-synthetic nonce on a previously reserved domain.
/// The owner must separately authorize and qualify any actual-host invocation.
/// The real adapter is not called during replay, duplicate creation, inspection
/// or uncertain publication. This API has no provider/arbitrary-executable path.
pub fn execute_nonce_domain(
    store: &ProgramStore,
    key: &StoreKey,
    backend: &mut SystemdNonceDomain,
    ids: &DomainOperations,
    nonce: &str,
    cancel: &AtomicBool,
) -> DomainRun {
    let start = Instant::now();
    if nonce != backend.selection.nonce {
        let mut r = DomainRun::empty();
        r.diagnostic = Some(issue(
            DomainPhaseDiagnostic::Preparation,
            DomainCause::Correspondence,
        ));
        return r;
    }
    let end = start.checked_add(Duration::from_millis(backend.selection.limits.wall_ms));
    let endpoint = store
        .load(key, Access::ReadOnly)
        .ok()
        .and_then(|k| record(&k, &ids.domain).ok())
        .and_then(|r| backend.endpoint(&r.intent.request).ok());
    match (end, endpoint) {
        (Some(end), Some(endpoint)) => execute(
            store,
            key,
            backend,
            ExecutionRequest {
                ids,
                nonce,
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
            let mut r = DomainRun::empty();
            r.diagnostic = Some(issue(
                DomainPhaseDiagnostic::Preparation,
                DomainCause::Contract,
            ));
            r
        }
    }
}

struct ExecutionRequest<'a> {
    ids: &'a DomainOperations,
    nonce: &'a str,
    cancel: &'a AtomicBool,
    start: Instant,
    end: Instant,
}

fn execute<M: DomainManager>(
    store: &ProgramStore,
    key: &StoreKey,
    manager: &mut M,
    request: ExecutionRequest<'_>,
    listener: impl FnOnce() -> Result<UnixListener, DomainDiagnostic>,
) -> DomainRun {
    let ExecutionRequest {
        ids,
        nonce,
        cancel,
        start,
        end,
    } = request;
    let mut result = DomainRun::empty();
    let attempt =
        || -> Result<(), DomainDiagnostic> {
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
                result.recovery_only = true;
                return Ok(());
            }
            manager.corresponds(&domain.intent)?;
            let at = current(&kernel, ids, start, end, cancel)?;
            let create = operation(
                &kernel,
                ids,
                ids.create.clone(),
                DomainEvent::CreateRequested {
                    domain: ids.domain.clone(),
                    at,
                },
            );
            let (receipt, created) = store
                .dispatch_domain(key, create, |k| {
                    current(k, ids, start, end, cancel)?;
                    let listener = listener()?;
                    listener.set_nonblocking(true).map_err(|e| {
                        issue(DomainPhaseDiagnostic::Creation, DomainCause::Io(e.kind()))
                    })?;
                    manager.create(&domain.intent, end)?;
                    Ok::<_, DomainDiagnostic>(listener)
                })
                .map_err(|_| issue(DomainPhaseDiagnostic::Publication, DomainCause::Journal))?;
            result.creation = Some(receipt);
            let Some(created) = created else {
                result.recovery_only = true;
                return Ok(());
            };
            let listener = created?;
            let mut channel = loop {
                current(
                    &store.load(key, Access::ReadOnly).map_err(|_| {
                        issue(DomainPhaseDiagnostic::Identity, DomainCause::Journal)
                    })?,
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
            let hello = frame_read(&mut channel, end)
                .map_err(|e| issue(DomainPhaseDiagnostic::Identity, DomainCause::Io(e.kind())))?;
            let actual = manager.observe(&domain.intent, end)?;
            if peer.pid.as_raw_pid() as u32 != actual.main_pid
                || !actual.populated
                || hello.as_array().is_none_or(|a| a.len() != 6)
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
            let kernel = store
                .load(key, Access::ReadOnly)
                .map_err(|_| issue(DomainPhaseDiagnostic::Publication, DomainCause::Journal))?;
            let at = current(&kernel, ids, start, end, cancel)?;
            let bound = operation(
                &kernel,
                ids,
                ids.bind.clone(),
                DomainEvent::Bound {
                    domain: ids.domain.clone(),
                    identity: actual.identity.clone(),
                    at,
                },
            );
            result.binding =
                Some(store.admit(key, bound).map_err(|_| {
                    issue(DomainPhaseDiagnostic::Publication, DomainCause::Journal)
                })?);
            let kernel = store
                .load(key, Access::ReadOnly)
                .map_err(|_| issue(DomainPhaseDiagnostic::Publication, DomainCause::Journal))?;
            let at = current(&kernel, ids, start, end, cancel)?;
            // A fresh live manager read follows durable publication. The previous
            // incarnation cannot authorize a replacement or a released/empty set.
            let before_release = manager.observe(&domain.intent, end)?;
            if before_release.identity != actual.identity
                || !before_release.populated
                || before_release.main_pid != actual.main_pid
                || before_release.start_ticks != actual.start_ticks
            {
                return Err(issue(
                    DomainPhaseDiagnostic::Release,
                    DomainCause::Correspondence,
                ));
            }
            let release = operation(
                &kernel,
                ids,
                ids.release.clone(),
                DomainEvent::ReleaseRequested {
                    domain: ids.domain.clone(),
                    identity: actual.identity.clone(),
                    at,
                },
            );
            let (receipt, released) = store
                .dispatch_domain(key, release, |k| {
                    current(k, ids, start, end, cancel)?;
                    frame_write(
                        &mut channel,
                        &json!([1, "release", domain.intent.request.as_str(), nonce]),
                        end,
                    )
                    .map_err(|e| issue(DomainPhaseDiagnostic::Release, DomainCause::Io(e.kind())))
                })
                .map_err(|_| issue(DomainPhaseDiagnostic::Publication, DomainCause::Journal))?;
            result.release = Some(receipt);
            let Some(released) = released else {
                result.recovery_only = true;
                return Ok(());
            };
            released?;
            let reply = frame_read(&mut channel, end)
                .map_err(|e| issue(DomainPhaseDiagnostic::Release, DomainCause::Io(e.kind())))?;
            if reply != json!([1, "nonce", domain.intent.request.as_str(), nonce]) {
                return Err(issue(DomainPhaseDiagnostic::Release, DomainCause::Protocol));
            }
            result.nonce = Some(nonce.into());
            let kernel = store
                .load(key, Access::ReadOnly)
                .map_err(|_| issue(DomainPhaseDiagnostic::Publication, DomainCause::Journal))?;
            let at = ids
                .at
                .checked_add(start.elapsed().as_millis() as u64)
                .ok_or_else(|| issue(DomainPhaseDiagnostic::Publication, DomainCause::Deadline))?;
            let report = operation(
                &kernel,
                ids,
                ids.report.clone(),
                DomainEvent::Report {
                    domain: ids.domain.clone(),
                    identity: actual.identity,
                    outcome: cortex_core::program::operational::EffectOutcome::Succeeded,
                    at,
                    basis: ids.basis.clone(),
                },
            );
            result.report =
                Some(store.admit(key, report).map_err(|_| {
                    issue(DomainPhaseDiagnostic::Publication, DomainCause::Journal)
                })?);
            Ok(())
        };
    if let Err(error) = attempt() {
        result.diagnostic = Some(error)
    }
    result
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

/// Persist one cancellation request before targeting the exact still-matching
/// manager incarnation. A stop request alone never records closure or a refund.
pub fn cancel_nonce_domain(
    store: &ProgramStore,
    key: &StoreKey,
    backend: &mut SystemdNonceDomain,
    operation: Operation,
) -> Result<Receipt, DomainDiagnostic> {
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
