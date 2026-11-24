//! Canonical executable-domain composition with independently managed lifetime.
//! No constructor call or source test constitutes permission for a host launch.
use super::{
    ExecutionError, ExecutionPlan,
    accounting::{
        CollectedCpuClosure, CollectedCpuClosureCut, CollectedCpuClosureError,
        CpuCounterObservation, ManagedCpuCut, ManagedCpuObservations, ManagedCpuReadback,
        challenge, clock_namespace, monotonic_ns, submit,
    },
    artifacts,
    backend::ExecutionObservation,
    bootstrap,
    cgroup::{Delegation, DelegationAuthorization, OwnedGroup},
    managed::ManagerRecoveryState,
    normal_end::{
        NormalEndClosureError, NormalEndContext, NormalEndCpuWitness, check_normal_end_cpu_closure,
    },
    transport,
};
use crate::{
    domain_manager::*,
    domain_runtime::{DomainFinish, DomainManager},
    native_linux::{NativeIsolation, StableProcess},
};
use cortex_core::{
    program::{
        OperationId,
        domain::*,
        operational::EffectOutcome,
        scheduling::Resource,
        supervision::{InterruptionReason, ProcessCompletion},
    },
    temporal::SourceReference,
};
use cortex_evidence::program::{CoordinationKernel, Operation};
use cortex_storage::{
    history_store::{StoreId, StoreKey},
    program_store::{AdmissionReceipt, ProgramStore},
};
use std::{
    fs::File,
    os::{
        fd::OwnedFd,
        unix::{fs::MetadataExt, net::UnixStream},
    },
    path::PathBuf,
    sync::atomic::Ordering,
    time::Instant,
};

fn diagnostic(error: ExecutionError) -> DomainDiagnostic {
    issue(
        DomainPhaseDiagnostic::Identity,
        match error {
            ExecutionError::Authority => DomainCause::Authority,
            ExecutionError::Io(k) => DomainCause::Io(k),
            _ => DomainCause::Correspondence,
        },
    )
}
/// Result of an exact managed tree read around bounded CPU counter reads.
/// A population snapshot does not establish quiescence, final CPU or settlement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManagedEndReadback {
    /// An exact tree or its continued correspondence could not be observed.
    Unavailable(DomainDiagnostic),
    /// One bounded readback from the held cgroup incarnation.
    Observed {
        /// Recursive population at the readback; false alone is no fence.
        populated: bool,
        /// Cumulative observation from the same held cgroup.
        cpu: CpuCounterObservation,
    },
}

impl ManagedEndReadback {
    fn populated(&self) -> Option<bool> {
        match self {
            Self::Observed { populated, .. } => Some(*populated),
            Self::Unavailable(_) => None,
        }
    }
}

/// In-memory managed end observation for one release. It neither enters the
/// journal nor supplies an independently witnessed closure or resource receipt.
pub struct ManagedEndObservation {
    /// Exact canonical domain for this attempt.
    pub domain: DomainId,
    /// Exact durable release-request operation.
    pub release: OperationId,
    /// Incarnation retained in the canonical binding. An unavailable readback
    /// does not establish that the incarnation was still present at observation.
    pub binding_identity: DomainIdentity,
    /// Monotonic cut at which the readback attempt returned.
    pub observed_at: Instant,
    /// Direct completion or interruption as recorded by the transport.
    pub completion: ProcessCompletion,
    /// Every input byte was accepted and both output streams reached bounded EOF.
    pub transport_complete: bool,
    /// Observed population/counter or the reason no exact readback was retained.
    pub readback: ManagedEndReadback,
}

struct AccountedTree {
    observation: ManagedObservation,
    nodes: Vec<DomainCgroupObservation>,
    cpu: CpuCounterObservation,
}

struct AccountedTreeRead {
    observation: ManagedObservation,
    nodes: Vec<DomainCgroupObservation>,
    reads: (Option<u64>, Option<u64>),
}

struct OpenCpuCapture {
    began_at: Instant,
    began_ns: Option<u64>,
    namespace: Option<(u64, u64)>,
}
impl OpenCpuCapture {
    fn start() -> Self {
        Self {
            began_at: Instant::now(),
            began_ns: monotonic_ns(),
            namespace: clock_namespace(),
        }
    }

    fn finish(
        self,
        cut: ManagedCpuCut,
        populated: Option<bool>,
        counter: CpuCounterObservation,
    ) -> ManagedCpuReadback {
        let clock_namespace = self
            .namespace
            .filter(|namespace| Some(*namespace) == clock_namespace());
        let monotonic_ns = self.began_ns.zip(monotonic_ns());
        ManagedCpuReadback {
            cut,
            began_at: self.began_at,
            observed_at: Instant::now(),
            monotonic_ns,
            clock_namespace,
            populated,
            counter,
        }
    }
}

/// Manager owns service lifetime and reaping; held delegation independently
/// establishes the permitted resource scope. Neither owns canonical authority.
pub struct ManagedExecutionDomain {
    plan: ExecutionPlan,
    manager: SystemdNonceDomain,
    delegation: Delegation,
    intent: Option<DomainIntent>,
    original_end: Option<Instant>,
    bound_group: Option<DomainIdentity>,
    group: Option<OwnedGroup>,
    scratch: Option<File>,
    observation: Option<ExecutionObservation>,
    end_observation: Option<ManagedEndObservation>,
    end_started_at: Option<Instant>,
    cpu_store: Option<(StoreId, OpenCpuCapture)>,
    cpu_observations: Option<ManagedCpuObservations>,
}
impl ManagedExecutionDomain {
    /// Read-only preparation of an explicitly authorized scope. None refuses
    /// before manager connection or pipe creation. This does not create a unit.
    pub fn inspect(
        plan: ExecutionPlan,
        selection: ServiceSelection,
        isolation: NativeIsolation,
        authorization: Option<DelegationAuthorization>,
    ) -> Result<Self, DomainDiagnostic> {
        let authorization = authorization.ok_or_else(|| diagnostic(ExecutionError::Delegation))?;
        if authorization.root != selection.delegated_root {
            return Err(diagnostic(ExecutionError::Identity));
        }
        validate_selection(&plan, &selection, &isolation).map_err(diagnostic)?;
        let delegation = Delegation::inspect(Some(authorization)).map_err(diagnostic)?;
        let manager = SystemdNonceDomain::inspect_executable(selection, isolation, plan.digest())?;
        Ok(Self {
            plan,
            manager,
            delegation,
            intent: None,
            original_end: None,
            bound_group: None,
            group: None,
            scratch: None,
            observation: None,
            end_observation: None,
            end_started_at: None,
            cpu_store: None,
            cpu_observations: None,
        })
    }
    /// Exact manager/policy/immutable source configuration for canonical intent.
    pub fn configuration(&self) -> &SourceReference {
        self.manager.configuration()
    }
    pub(crate) fn observation_end(&self) -> Option<Instant> {
        self.original_end
    }
    pub(crate) fn bind_cpu_store(
        &mut self,
        store: StoreId,
        started_at: Instant,
    ) -> Result<(), DomainDiagnostic> {
        self.check_bound_store(store)?;
        if self.intent.is_none() {
            self.cpu_store = Some((
                store,
                OpenCpuCapture {
                    began_at: started_at,
                    began_ns: monotonic_ns(),
                    namespace: clock_namespace(),
                },
            ));
        }
        Ok(())
    }
    pub(crate) fn check_bound_store(&self, store: StoreId) -> Result<(), DomainDiagnostic> {
        if self
            .cpu_store
            .as_ref()
            .is_some_and(|(bound, _)| *bound != store)
        {
            return Err(diagnostic(ExecutionError::Identity));
        }
        Ok(())
    }
    pub(super) fn check_p1_selection(
        &self,
        root: &DelegationAuthorization,
        manager: &str,
        version: &str,
        end: Instant,
    ) -> Result<(), DomainDiagnostic> {
        self.delegation
            .check_selected_root(root)
            .map_err(diagnostic)?;
        self.manager.check_p1_manager(manager, version, end)
    }

    pub(super) fn plan(&self) -> &ExecutionPlan {
        &self.plan
    }

    pub(crate) fn wall_ms(&self) -> u64 {
        self.plan.spec.limits.wall_ms
    }
    pub(crate) fn digest(&self) -> &str {
        self.plan.digest()
    }
    pub(crate) fn endpoint(
        &self,
        request: &cortex_core::program::OperationId,
    ) -> Result<PathBuf, DomainDiagnostic> {
        self.manager.endpoint(request)
    }
    pub(crate) fn take_observation(&mut self) -> Option<ExecutionObservation> {
        self.observation.take()
    }
    /// Return the retained in-memory observation after a managed transport finishes.
    /// Reading it does not contact the manager or refresh the observed cut.
    pub fn end_observation(&self) -> Option<&ManagedEndObservation> {
        self.end_observation.as_ref()
    }
    /// Return the live held group's cumulative CPU observations. The first cut
    /// occurs after bootstrap startup. Neither it nor an empty later tree proves
    /// the initial population, complete attribution or terminal consumption.
    pub fn cpu_observations(&self) -> Option<&ManagedCpuObservations> {
        self.cpu_observations.as_ref()
    }
    /// Read the exact operation and collector-cut binding for the selected
    /// whole-domain observer. This digest provides correspondence, not authority.
    pub fn cpu_closure_challenge(
        &self,
        request: &CollectedCpuClosure,
    ) -> Result<[u8; 32], CollectedCpuClosureError> {
        challenge(self.cpu_closure_history(request.cut)?, request)
    }

    /// Offer the collector-derived CPU debit to the existing canonical store.
    ///
    /// The selected independent observer must send a complete fence report over a
    /// connected Unix seqpacket socket. Linux peer/packet credentials and the
    /// protected trust configuration determine the observer role. The report
    /// authenticates coverage and the exact selected cut. The CPU amount comes
    /// only from the held counter. No other dimension is released here.
    pub fn admit_collected_cpu_closure(
        &self,
        store: &ProgramStore,
        key: &StoreKey,
        request: CollectedCpuClosure,
        observer: OwnedFd,
    ) -> Result<AdmissionReceipt, CollectedCpuClosureError> {
        submit(
            store,
            key,
            self.cpu_closure_history(request.cut)?,
            self.intent
                .as_ref()
                .ok_or(CollectedCpuClosureError::Correspondence)?,
            request,
            observer,
        )
    }

    fn cpu_closure_history(
        &self,
        cut: CollectedCpuClosureCut,
    ) -> Result<&ManagedCpuObservations, CollectedCpuClosureError> {
        if matches!(cut, CollectedCpuClosureCut::NormalEnd)
            && !self.end_observation.as_ref().is_some_and(|observation| {
                successful_managed_end(
                    &observation.completion,
                    observation.transport_complete,
                    &observation.readback,
                )
            })
        {
            return Err(CollectedCpuClosureError::Counter);
        }
        self.cpu_observations
            .as_ref()
            .ok_or(CollectedCpuClosureError::Counter)
    }
    /// Screen an independently supplied normal-end Closed/Settle candidate.
    /// The return is the rounded cumulative CPU amount, subject to the actual
    /// journal's current admission and separate witness authentication. This
    /// does not commit the operation or establish any physical terminal fact.
    pub fn check_normal_end_cpu_closure(
        &self,
        kernel: &CoordinationKernel,
        witness: Option<&NormalEndCpuWitness>,
        operation: &Operation,
    ) -> Result<u64, NormalEndClosureError> {
        let missing = NormalEndClosureError::MissingContext;
        let context = NormalEndContext {
            intent: self.intent.as_ref().ok_or(missing.clone())?,
            observation: self.end_observation.as_ref().ok_or(missing.clone())?,
            start: self.end_started_at.ok_or(missing.clone())?,
            end: self.original_end.ok_or(missing)?,
        };
        check_normal_end_cpu_closure(kernel, context, witness, operation)
    }

    /// Read-only exact-incarnation recovery. No channel, unit, release, stop or
    /// resource settlement is reconstructed. Caller supplies the recorded end.
    pub fn inspect_recovery(
        &mut self,
        intent: &DomainIntent,
        identity: &DomainIdentity,
        end: Instant,
    ) -> ManagerRecoveryState {
        if let Err(error) = self.delegation.check().map_err(diagnostic) {
            return ManagerRecoveryState::Unavailable(error);
        }
        self.manager.recover_execution(intent, identity, end)
    }
    fn accounted_tree(
        &mut self,
        intent: &DomainIntent,
        expected: &DomainIdentity,
        end: Instant,
        cut: ManagedCpuCut,
    ) -> Result<AccountedTree, DomainDiagnostic> {
        let end = end.min(
            self.original_end
                .ok_or_else(|| diagnostic(ExecutionError::Identity))?,
        );
        let capture = OpenCpuCapture::start();
        let result = self.read_accounted_tree(intent, expected, end);
        let mut sample = capture.finish(cut, None, CpuCounterObservation::Unavailable);
        let result = result.and_then(|read| {
            if sample.observed_at < end {
                Ok(read)
            } else {
                Err(issue(DomainPhaseDiagnostic::Closure, DomainCause::Deadline))
            }
        });
        let (populated, reads) = result.as_ref().map_or((None, (None, None)), |read| {
            (Some(read.observation.populated), read.reads)
        });
        sample.populated = populated;
        let cpu = match &mut self.cpu_observations {
            Some(history) => history.record_bracket(expected, cut, sample, reads),
            None => CpuCounterObservation::Unavailable,
        };
        result.map(|read| AccountedTree {
            observation: read.observation,
            nodes: read.nodes,
            cpu,
        })
    }
    fn read_accounted_tree(
        &mut self,
        intent: &DomainIntent,
        expected: &DomainIdentity,
        end: Instant,
    ) -> Result<AccountedTreeRead, DomainDiagnostic> {
        if Instant::now() >= end {
            return Err(issue(DomainPhaseDiagnostic::Closure, DomainCause::Deadline));
        }
        if self.intent.as_ref() != Some(intent) || self.bound_group.as_ref() != Some(expected) {
            return Err(diagnostic(ExecutionError::Identity));
        }
        let group = self
            .group
            .as_ref()
            .ok_or_else(|| diagnostic(ExecutionError::Identity))?;
        let before = group.cpu_usage(expected).map_err(diagnostic)?;
        if Instant::now() >= end {
            return Err(issue(DomainPhaseDiagnostic::Closure, DomainCause::Deadline));
        }
        let (actual, tree) = self.manager.observe_tree(intent, end)?;
        if &actual.identity != expected {
            return Err(diagnostic(ExecutionError::Identity));
        }
        if Instant::now() >= end {
            return Err(issue(DomainPhaseDiagnostic::Closure, DomainCause::Deadline));
        }
        let after = group.cpu_usage(expected).map_err(diagnostic)?;
        if Instant::now() >= end {
            return Err(issue(DomainPhaseDiagnostic::Closure, DomainCause::Deadline));
        }
        Ok(AccountedTreeRead {
            observation: actual,
            nodes: tree,
            reads: (before, after),
        })
    }
    fn retain_scratch(&mut self, actual: &ManagedObservation) -> Result<(), DomainDiagnostic> {
        let stable = StableProcess::acquire(actual.main_pid, actual.start_ticks)
            .map_err(|e| diagnostic(e.into()))?;
        stable.require_live().map_err(|e| diagnostic(e.into()))?;
        let scratch = File::open(format!("/proc/{}/root/scratch", actual.main_pid))
            .map_err(|e| diagnostic(e.into()))?;
        if rustix::fs::fstatfs(&scratch)
            .map_err(|e| diagnostic(e.into()))?
            .f_type as u64
            != 0x01021994
        {
            return Err(diagnostic(ExecutionError::Identity));
        }
        stable.require_live().map_err(|e| diagnostic(e.into()))?;
        if let Some(old) = &self.scratch {
            let a = old.metadata().map_err(|e| diagnostic(e.into()))?;
            let b = scratch.metadata().map_err(|e| diagnostic(e.into()))?;
            if (a.dev(), a.ino()) != (b.dev(), b.ino()) {
                return Err(diagnostic(ExecutionError::Identity));
            }
        } else {
            self.scratch = Some(scratch);
        }
        Ok(())
    }
}
fn validate_selection(
    plan: &ExecutionPlan,
    s: &ServiceSelection,
    isolation: &NativeIsolation,
) -> Result<(), ExecutionError> {
    let l = &s.limits;
    let c = &plan.spec.controls;
    if s.nonce != plan.digest()
        || l.wall_ms == 0
        || l.wall_ms > plan.spec.limits.wall_ms
        || plan.spec.limits.wall_ms > 15000
        || plan.spec.limits.cleanup_ms < 500
        || l.cpu_quota_us != c.cpu_quota_us
        || l.cpu_period_us != c.cpu_period_us
        || l.memory_bytes != c.memory_bytes
        || l.swap_bytes != c.swap_bytes
        || l.tasks != c.tasks_max
        || isolation.scratch_limits() != (plan.spec.scratch_bytes, plan.spec.scratch_inodes)
    {
        return Err(ExecutionError::Policy);
    }
    Ok(())
}

// The caller checks current authority separately. The projection supplies the
// admitted amount and whether that same domain still holds it.
pub(crate) fn check_scratch_reservation(
    domains: &DomainProjection,
    intent: &DomainIntent,
    scratch_bytes: u64,
) -> Result<(), ExecutionError> {
    let record = domains
        .domain(&intent.id)
        .filter(|record| &record.intent == intent)
        .ok_or(ExecutionError::Authority)?;
    let reserved_bytes = record
        .intent
        .effect
        .demand
        .iter()
        .find(|demand| demand.resource == Resource::DiskBytes)
        .and_then(|demand| demand.amount);
    if scratch_bytes == 0
        || !record.holds(Resource::DiskBytes)
        || reserved_bytes.is_none_or(|bytes| bytes < scratch_bytes)
    {
        return Err(ExecutionError::Policy);
    }
    Ok(())
}

impl DomainManager for ManagedExecutionDomain {
    fn check_context(
        &self,
        k: &CoordinationKernel,
        i: &DomainIntent,
        at: u64,
    ) -> Result<(), DomainDiagnostic> {
        self.delegation.check().map_err(diagnostic)?;
        let isolation = self
            .manager
            .execution_isolation()
            .ok_or_else(|| diagnostic(ExecutionError::Policy))?;
        isolation.check().map_err(|e| diagnostic(e.into()))?;
        self.manager.corresponds(i)?;
        self.plan
            .check_with_configuration(k, i, at, self.configuration())
            .map_err(diagnostic)?;
        let domains = k
            .domains()
            .ok_or_else(|| diagnostic(ExecutionError::Authority))?;
        check_scratch_reservation(domains, i, isolation.scratch_limits().0).map_err(diagnostic)
    }
    fn corresponds(&self, i: &DomainIntent) -> Result<(), DomainDiagnostic> {
        self.delegation.check().map_err(diagnostic)?;
        self.manager.corresponds(i)
    }
    fn is_nonce(&self) -> bool {
        false
    }
    fn isolation(&self) -> Option<&NativeIsolation> {
        self.manager.execution_isolation()
    }
    fn verify_ready(&self, hello: &serde_json::Value) -> bool {
        self.manager.verify_ready(hello)
    }
    fn pin_process(
        &self,
        a: &ManagedObservation,
    ) -> Result<Option<StableProcess>, DomainDiagnostic> {
        StableProcess::acquire(a.main_pid, a.start_ticks)
            .map(Some)
            .map_err(|e| diagnostic(e.into()))
    }
    fn create(&mut self, i: &DomainIntent, end: Instant) -> Result<(), DomainDiagnostic> {
        if self.intent.is_some() {
            return Err(diagnostic(ExecutionError::Identity));
        }
        // Preserve the attempted identity before submission, including lost reply.
        self.intent = Some(i.clone());
        self.original_end = Some(end);
        self.manager.create(i, end)
    }
    fn setup(&mut self, channel: &mut UnixStream, end: Instant) -> Result<(), DomainDiagnostic> {
        let intent = self
            .intent
            .as_ref()
            .ok_or_else(|| diagnostic(ExecutionError::Identity))?;
        let actual = self
            .manager
            .transfer_execution_pipes(intent, channel, end)?;
        let peer =
            rustix::net::sockopt::socket_peercred(&*channel).map_err(|e| diagnostic(e.into()))?;
        if peer.pid.as_raw_pid() as u32 != actual.main_pid {
            return Err(diagnostic(ExecutionError::Identity));
        }
        let group = self
            .delegation
            .inspect_bound_group(&actual.identity, &self.plan.spec.controls)
            .map_err(diagnostic)?;
        let (store, start) = self
            .cpu_store
            .as_ref()
            .ok_or_else(|| diagnostic(ExecutionError::Identity))?;
        let capture = OpenCpuCapture::start();
        let held_counter = group.cpu_usage(&actual.identity).map_err(diagnostic)?;
        let first_held = capture.finish(
            ManagedCpuCut::FirstHeld,
            Some(actual.populated),
            held_counter.map_or(
                CpuCounterObservation::Unavailable,
                CpuCounterObservation::LowerBoundMicros,
            ),
        );
        if first_held.observed_at >= end {
            return Err(issue(
                DomainPhaseDiagnostic::Preparation,
                DomainCause::Deadline,
            ));
        }
        self.cpu_observations = Some(ManagedCpuObservations::new(
            *store,
            intent.id.clone(),
            actual.identity.clone(),
            start.began_at,
            start
                .began_ns
                .filter(|_| start.namespace == first_held.clock_namespace),
            end,
            first_held,
        ));
        self.group = Some(group);
        self.bound_group = Some(actual.identity.clone());
        let c = &self.plan.spec.controls;
        let path = PathBuf::from("/sys/fs/cgroup")
            .join(actual.identity.control_group.trim_start_matches('/'));
        crate::domain_bootstrap::frame_write(
            channel,
            &serde_json::json!([
                1,
                "setup",
                self.plan.digest(),
                self.plan.spec.scratch_bytes,
                self.plan.spec.scratch_inodes,
                self.plan
                    .spec
                    .files
                    .iter()
                    .map(bootstrap::file_value)
                    .collect::<Vec<_>>(),
                self.plan.spec.arguments,
                self.plan.spec.environment,
                self.plan.spec.working_directory,
                [
                    path.to_str()
                        .ok_or_else(|| diagnostic(ExecutionError::Identity))?
                        .to_owned(),
                    actual.identity.cgroup_device.to_string(),
                    actual.identity.cgroup_inode.to_string(),
                    actual.identity.boot,
                    c.cpu_quota_us.to_string(),
                    c.cpu_period_us.to_string(),
                    c.memory_bytes.to_string(),
                    c.swap_bytes.to_string(),
                    c.tasks_max.to_string()
                ]
            ]),
            end,
        )
        .map_err(|e| diagnostic(e.into()))
    }
    fn observe(
        &mut self,
        i: &DomainIntent,
        end: Instant,
    ) -> Result<ManagedObservation, DomainDiagnostic> {
        let actual = self.manager.observe(i, end)?;
        let group = self
            .group
            .as_ref()
            .ok_or_else(|| diagnostic(ExecutionError::Identity))?;
        if group.identity() != (actual.identity.cgroup_device, actual.identity.cgroup_inode) {
            return Err(diagnostic(ExecutionError::Identity));
        }
        group.check_controls().map_err(diagnostic)?;
        self.retain_scratch(&actual)?;
        Ok(actual)
    }
    fn observe_tree(
        &mut self,
        intent: &DomainIntent,
        end: Instant,
    ) -> Result<(ManagedObservation, Vec<DomainCgroupObservation>), DomainDiagnostic> {
        let identity = self
            .bound_group
            .clone()
            .ok_or_else(|| diagnostic(ExecutionError::Identity))?;
        let mut tree =
            self.accounted_tree(intent, &identity, end, ManagedCpuCut::AfterCancellation)?;
        tree.observation.cpu_micros = tree.cpu.lower_bound_micros();
        Ok((tree.observation, tree.nodes))
    }
    fn stop(
        &mut self,
        i: &DomainIntent,
        identity: &DomainIdentity,
        end: Instant,
    ) -> Result<(), DomainDiagnostic> {
        if self.intent.as_ref() != Some(i) || Instant::now() >= end {
            return Err(diagnostic(ExecutionError::Identity));
        }
        let actual = self.manager.observe_tree(i, end)?.0;
        let group = self
            .group
            .as_ref()
            .ok_or_else(|| diagnostic(ExecutionError::Identity))?;
        if actual.identity != *identity
            || group.identity() != (identity.cgroup_device, identity.cgroup_inode)
        {
            return Err(diagnostic(ExecutionError::Identity));
        }
        // No name-based StopUnit request: concurrent name reuse cannot redirect
        // this write away from the held, verified kernel cgroup incarnation.
        // Acknowledgment is not observed quiescence, closure or refund.
        group.request_termination().map_err(diagnostic)
    }
    fn finish(
        &mut self,
        _channel: &mut UnixStream,
        context: DomainFinish<'_>,
        authority: &mut dyn FnMut() -> Result<(), DomainDiagnostic>,
    ) -> Result<EffectOutcome, DomainDiagnostic> {
        let pipes = self.manager.take_execution_pipes()?;
        let mut readback_error = None;
        let manager = &mut self.manager;
        let mut poll = |end| match manager.execution_completion(
            context.intent,
            &context.binding.identity,
            end,
        ) {
            Ok(result) => Ok(result),
            Err(error) => {
                readback_error = Some(error);
                Err(std::io::Error::other("manager observation unavailable"))
            }
        };
        let mut current = || {
            if context.cancel.load(Ordering::SeqCst) {
                return Err(InterruptionReason::Cancelled);
            }
            authority().map_err(|_| InterruptionReason::AuthorityUnknown)
        };
        let (streams, summary) = transport::collect_managed(
            pipes,
            self.plan.input(),
            &self.plan.spec.limits,
            context.start,
            context.end,
            &mut current,
            &mut poll,
            self.plan.uses_json_lines(),
        )
        .map_err(diagnostic)?;
        let readback = match self.accounted_tree(
            context.intent,
            &context.binding.identity,
            context.end,
            ManagedCpuCut::EndOfTransport,
        ) {
            Ok(tree) => ManagedEndReadback::Observed {
                populated: tree.observation.populated,
                cpu: tree.cpu,
            },
            Err(error) => ManagedEndReadback::Unavailable(error),
        };
        let populated = readback.populated();
        let transport_complete =
            streams.input_complete && streams.stdout_complete && streams.stderr_complete;
        let succeeded = successful_managed_end(&streams.completion, transport_complete, &readback);
        self.end_started_at = Some(context.start);
        self.end_observation = Some(ManagedEndObservation {
            domain: context.intent.id.clone(),
            release: context.release.clone(),
            binding_identity: context.binding.identity.clone(),
            observed_at: Instant::now(),
            completion: streams.completion.clone(),
            transport_complete,
            readback,
        });
        let artifacts = if populated == Some(false) {
            artifacts::capture(
                self.scratch
                    .as_ref()
                    .ok_or_else(|| diagnostic(ExecutionError::Identity))?,
                &self.plan.spec.artifacts,
                self.plan.spec.scratch_bytes,
            )
            .map_err(diagnostic)?
        } else {
            Vec::new()
        };
        let structured = summary
            .map(|s| {
                crate::structured_output::BoundStructuredObservation::new(
                    context.intent,
                    context.binding,
                    context.release,
                    s,
                )
            })
            .transpose()
            .map_err(|_| diagnostic(ExecutionError::Identity))?;
        self.observation = Some(ExecutionObservation {
            streams,
            artifacts,
            domain_populated: populated,
            structured,
        });
        if let Some(error) = readback_error {
            return Err(error);
        }
        Ok(if succeeded {
            EffectOutcome::Succeeded
        } else {
            EffectOutcome::Interrupted
        })
    }
}

// Delimit a process report. Neither a matching exit nor an empty readback
// makes the observation a terminal closure or an accounting receipt.
pub(super) fn successful_managed_end(
    completion: &ProcessCompletion,
    transport_complete: bool,
    readback: &ManagedEndReadback,
) -> bool {
    matches!(
        completion,
        ProcessCompletion::Exited {
            code: Some(0),
            signal: None
        }
    ) && transport_complete
        && readback.populated() == Some(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn managed_exit_cannot_hide_population_missing_readback_or_incomplete_streams() {
        let success = ProcessCompletion::Exited {
            code: Some(0),
            signal: None,
        };
        let evidence = |populated, cpu| ManagedEndReadback::Observed { populated, cpu };
        let observed = evidence(false, CpuCounterObservation::LowerBoundMicros(1001));
        assert!(successful_managed_end(&success, true, &observed));
        assert!(!successful_managed_end(&success, false, &observed));

        let still_populated = evidence(true, CpuCounterObservation::LowerBoundMicros(1001));
        assert!(!successful_managed_end(&success, true, &still_populated));
        let changed_epoch = ManagedEndReadback::Unavailable(issue(
            DomainPhaseDiagnostic::Closure,
            DomainCause::Correspondence,
        ));
        assert!(!successful_managed_end(&success, true, &changed_epoch));

        let unknown_counter = evidence(false, CpuCounterObservation::Unavailable);
        assert!(successful_managed_end(&success, true, &unknown_counter));
        let cancelled = ProcessCompletion::Interrupted {
            reason: InterruptionReason::Cancelled,
        };
        assert!(!successful_managed_end(&cancelled, true, &unknown_counter));
    }
}
