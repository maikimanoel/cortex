//! Recoverable execution-domain facts and shared-pool accounting.
//!
//! This projection has no I/O. The explicit version-four coordinator journal is
//! its only admission owner. Receipt fields are trusted-operator observations,
//! not signatures or host qualification. A closure changes present capability;
//! it never changes the historical result or grants permission to retry.
use super::{operational::*, scheduling::*, *};
use std::collections::BTreeSet;

/// Exact durable domain identity, distinct from an effect or a manager name.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct DomainId(String);
impl DomainId {
    /// Validate a bounded identifier.
    pub fn new(value: &str) -> Result<Self, ProgramError> {
        crate::validate_identifier("domain", value)?;
        Ok(Self(value.into()))
    }
    /// Inspect the supplied identifier.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
/// Physical pool correspondence supplied by the operator; labels are not proof.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoolIdentity {
    /// Stable physical allocation reference, not a program-local alias.
    pub physical: String,
    /// Allocation incarnation.
    pub incarnation: String,
    /// Host identity reference.
    pub host: String,
    /// Exact boot identity.
    pub boot: String,
}
impl PoolIdentity {
    /// Reject missing, unbounded or control-bearing identity assertions.
    pub fn validate(&self) -> Result<(), DomainError> {
        for s in [&self.physical, &self.incarnation, &self.host, &self.boot] {
            text(s)?;
        }
        Ok(())
    }
}
/// Consumption units are separate from reusable milli-CPU and RAM capacity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Consumption {
    /// Cumulative scheduled CPU execution in milliseconds.
    CpuExecutionMillis,
    /// Cumulative elapsed allocation in milliseconds.
    ElapsedMillis,
    /// Cumulative delivered bytes.
    InputBytes,
    /// Cumulative output bytes.
    OutputBytes,
    /// Cumulative recorded events.
    Events,
}
/// Explicit finite cumulative policy; no reservation is a proven usage bound.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsumptionBudget {
    /// Dimension.
    pub dimension: Consumption,
    /// Total finite allowance, including protected reserve.
    pub total: u64,
    /// Unavailable to governed work.
    pub reserve: u64,
    /// Maximum declared reservation per operation.
    pub per_operation: u64,
}
/// Declared allocation, independently tracked from actual measured use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsumptionReservation {
    /// Dimension.
    pub dimension: Consumption,
    /// Finite requested debit.
    pub amount: u64,
}
/// One authoritative pool per explicit coordinator journal.
#[derive(Clone, PartialEq, Eq)]
pub struct PoolContract {
    /// Physical pool and incarnation.
    pub identity: PoolIdentity,
    /// Exact operator-approved journal association and resource contract.
    pub basis: SourceReference,
    /// Reusable dimensions. Available/reserve are policy, not enforcement proof.
    pub capacities: Vec<Capacity>,
    /// Required cumulative dimensions; unknown debt blocks further admission.
    pub consumption: Vec<ConsumptionBudget>,
    /// Maximum retained domains, including closed and unresolved ones.
    pub max_domains: usize,
}
/// Immutable pre-creation contract. A manager request may be submitted once.
#[derive(Clone, PartialEq, Eq)]
pub struct DomainIntent {
    /// Never reused, including after closure.
    pub id: DomainId,
    /// Existing candidate, package, claim and operational-grant correspondence.
    pub effect: EffectIntent,
    /// Exact registered pool.
    pub pool: PoolIdentity,
    /// Unique creation request, durable before manager interaction.
    pub request: OperationId,
    /// Exact bootstrap/isolation/payload configuration reference.
    pub configuration: SourceReference,
    /// Explicit cumulative allocation, not a measured upper bound.
    pub consumption: Vec<ConsumptionReservation>,
}
/// Actual managed domain incarnation; a name or changed boot alone is insufficient.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainIdentity {
    /// Expected pool allocation.
    pub pool: PoolIdentity,
    /// Exact creation request.
    pub request: OperationId,
    /// Exact unique manager bus owner or equivalent incarnation.
    pub manager: String,
    /// Manager's invocation incarnation.
    pub invocation: String,
    /// Exact control-group path.
    pub control_group: String,
    /// Control-group filesystem device identity.
    pub cgroup_device: u64,
    /// Control-group inode identity.
    pub cgroup_inode: u64,
    /// Host boot at observation.
    pub boot: String,
}
impl DomainIdentity {
    /// Validate structural identity, without authenticating observations.
    pub fn validate(&self) -> Result<(), DomainError> {
        self.pool.validate()?;
        for s in [
            &self.manager,
            &self.invocation,
            &self.control_group,
            &self.boot,
        ] {
            text(s)?;
        }
        if self.boot != self.pool.boot
            || self.cgroup_inode == 0
            || !self.control_group.starts_with('/')
        {
            return Err(DomainError::Correspondence);
        }
        Ok(())
    }
}
/// One observed process incarnation. A PID alone is not an identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainProcessObservation {
    /// Kernel process identifier at observation.
    pub pid: u32,
    /// Process birth tick from the same boot's procfs.
    pub start_ticks: u64,
}
/// Observed controls on the exact managed domain, not qualification evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainControlReadback {
    /// CPU quota per period, in microseconds; not cumulative execution.
    pub cpu_quota_us: u64,
    /// CPU quota period in microseconds.
    pub cpu_period_us: u64,
    /// Charged-memory ceiling in bytes.
    pub memory_bytes: u64,
    /// Swap ceiling in bytes.
    pub swap_bytes: u64,
    /// Tasks/threads ceiling.
    pub tasks: u64,
    /// Manager runtime deadline in microseconds.
    pub runtime_max_micros: u64,
    /// Manager stop-action timeout in microseconds, not quiescence proof.
    pub timeout_stop_micros: u64,
    /// Manager restart policy forbids automatic restart.
    pub restart_prevented: bool,
    /// Stop policy covers the manager's control group.
    pub kill_control_group: bool,
    /// The service forbids acquiring new privileges.
    pub no_new_privileges: bool,
    /// Manager escalation is enabled.
    pub send_sigkill: bool,
    /// The fixed service retains its unit after its main process exits.
    pub remain_after_exit: bool,
}
/// Immutable post-create binding evidence. Direct members are a bounded snapshot,
/// not proof of descendants, atomic capture, cleanup, or snapshot exclusion.
#[derive(Clone, PartialEq, Eq)]
pub struct DomainBindingObservation {
    /// Actual domain incarnation.
    pub identity: DomainIdentity,
    /// Exact intended immutable executable/configuration reference.
    pub configuration: SourceReference,
    /// Main process whose bootstrap channel was observed.
    pub main: DomainProcessObservation,
    /// Direct cgroup members with observed birth ticks and membership.
    pub direct_members: Vec<DomainProcessObservation>,
    /// Retained effective readbacks, independently from requested limits.
    pub controls: DomainControlReadback,
    /// Observed cgroup population at binding.
    pub populated: bool,
    /// Mechanism/version provenance, not authenticated capture or acceptance.
    pub basis: SourceReference,
}
impl DomainBindingObservation {
    /// Validate bounded correspondence without authenticating host assertions.
    pub fn validate(&self) -> Result<(), DomainError> {
        self.identity.validate()?;
        let c = &self.controls;
        if !self.populated
            || self.main.pid == 0
            || self.main.start_ticks == 0
            || self.direct_members.is_empty()
            || self.direct_members.len() > 8
            || self.direct_members.len() as u64 > c.tasks
            || c.tasks == 0
            || c.tasks > 8
            || c.cpu_quota_us == 0
            || c.cpu_period_us == 0
            || c.memory_bytes == 0
            || c.swap_bytes != 0
            || c.runtime_max_micros == 0
            || c.timeout_stop_micros == 0
            || !c.restart_prevented
            || !c.kill_control_group
            || !c.no_new_privileges
            || !c.send_sigkill
            || !c.remain_after_exit
            || !self.direct_members.contains(&self.main)
        {
            return Err(DomainError::Structure);
        }
        let mut seen = BTreeSet::new();
        for member in &self.direct_members {
            if member.pid == 0 || member.start_ticks == 0 || !seen.insert(member.pid) {
                return Err(DomainError::Structure);
            }
        }
        Ok(())
    }
}
/// Bounded post-cancellation readback of the same native domain. These facts
/// do not cover the whole subtree, helpers, retained streams or snapshot safety.
/// They never become a closure receipt or final consumption merely by replay.
#[derive(Clone, PartialEq, Eq)]
pub struct DomainClosureObservation {
    /// Exact actual manager, boot and cgroup incarnation.
    pub identity: DomainIdentity,
    /// Original immutable configuration.
    pub configuration: SourceReference,
    /// Exact program/effect attempt and generation correspondence.
    pub scope: EffectScope,
    /// Birth identity retained at the original channel binding.
    pub binding_main: DomainProcessObservation,
    /// Current main process if still observable, never guessed from the old PID.
    pub observed_main: Option<DomainProcessObservation>,
    /// Bounded direct members only; an empty list is not whole-domain proof.
    pub direct_members: Vec<DomainProcessObservation>,
    /// Kernel population readback, independent of parent exit or stop reply.
    pub populated: bool,
    /// Current restart policy readback, not complete resurrection prevention.
    pub restart_prevented: bool,
    /// Sampled CPU use in microseconds, not complete final consumption.
    pub cpu_micros: Option<u64>,
    /// Charged-memory peak readback, not reusable capacity disposition.
    pub memory_peak: Option<u64>,
    /// Logical observation tick, retained unchanged at publication and replay.
    pub observed_at: u64,
    /// Mechanism/version provenance; caller assertions are not authenticated.
    pub basis: SourceReference,
}
impl DomainClosureObservation {
    /// Validate the bounded sample without treating it as physical closure proof.
    pub fn validate(&self) -> Result<(), DomainError> {
        self.identity.validate()?;
        if self.binding_main.pid == 0
            || self.binding_main.start_ticks == 0
            || self.direct_members.len() > 8
            || (!self.populated && !self.direct_members.is_empty())
        {
            return Err(DomainError::Structure);
        }
        let mut seen = BTreeSet::new();
        for member in self.direct_members.iter().chain(self.observed_main.iter()) {
            if member.pid == 0 || member.start_ticks == 0 {
                return Err(DomainError::Structure);
            }
            if member.pid == self.binding_main.pid && member != &self.binding_main {
                return Err(DomainError::Correspondence);
            }
        }
        for member in &self.direct_members {
            if !seen.insert(member.pid) {
                return Err(DomainError::Structure);
            }
        }
        if self
            .observed_main
            .as_ref()
            .is_some_and(|main| main != &self.binding_main || !self.direct_members.contains(main))
        {
            return Err(DomainError::Correspondence);
        }
        Ok(())
    }
}
/// Missing proof within the native direct-member observation boundary. These
/// reasons do not reinterpret a separately retained legacy closure receipt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClosureProofGap {
    /// No matching post-create channel binding was retained.
    BindingObservation,
    /// Cancellation has not been durably requested.
    DurableCancellation,
    /// No matching post-cancellation readback was retained.
    ClosureObservation,
    /// Population still indicates possible live effects.
    PopulatedDomain,
    /// A direct member or main process remains observable.
    RemainingMembers,
    /// Restart prevention was not observed.
    RestartPrevention,
    /// Direct membership does not establish complete descendant coverage.
    WholeDomainCoverage,
    /// Launcher and helper coverage is not established by this readback.
    HelperCoverage,
    /// Retained handles and streams are outside this observation boundary.
    StreamCoverage,
    /// Snapshot/restoration exclusion has no applicable trusted proof.
    SnapshotExclusion,
    /// Sampled counters are not final, complete cumulative accounting.
    FinalConsumption,
}
/// Read-only native proof-gap projection. It grants no closure, settlement,
/// retry or authority; a retained receipt remains separately caller-asserted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainClosureInspection {
    /// Number of immutable partial readbacks retained (at most 32).
    pub retained_observations: usize,
    /// A separate legacy closure decision already exists in the journal.
    pub receipt_recorded: bool,
    /// Properties not established by the latest native readback.
    pub missing: Vec<ClosureProofGap>,
}
/// Independent historical outcome. Closure never writes this field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoricalOutcome {
    /// No externally meaningful request yet.
    NotDispatched,
    /// Creation/release/result is unresolved.
    Unknown,
    /// Captured terminal observation, not acceptance.
    Reported(EffectOutcome),
}
/// Present effect capability, distinct from outcome and administrative intent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DomainPhase {
    /// Durable contract only.
    Reserved,
    /// Creation may have occurred; never repeat submission.
    CreateUncertain,
    /// Actual incarnation durably bound before payload release.
    Bound,
    /// Release may have occurred; never retransmit blindly.
    ReleaseUncertain,
    /// Applicable closure proof retained.
    Closed,
    /// Cancelled before any durable creation intent; no external request is permitted.
    CancelledBeforeCreate,
}
/// Administrative decisions neither close a domain nor refund resources.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdministrativeDisposition {
    /// Continue only with current authority.
    Active,
    /// Hold further work.
    Paused,
    /// Retain original facts and debts.
    Abandoned,
}
/// Measured final consumption; absence retains unbounded debt, not zero.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsumptionObservation {
    /// Exact dimension.
    pub dimension: Consumption,
    /// Complete measured amount, or unresolved.
    pub measured: Option<u64>,
}
/// Exact whole-domain closure assertion with explicit coverage and safe recovery.
/// Authentication/independence remains an adapter and owner obligation.
#[derive(Clone, PartialEq, Eq)]
pub struct ClosureReceipt {
    /// Exact retained domain.
    pub domain: DomainId,
    /// Exact actual incarnation; never just a missing name.
    pub identity: DomainIdentity,
    /// Immutable configuration correspondence.
    pub configuration: SourceReference,
    /// Exact original scope, including candidate/attempt/generation/package.
    pub scope: EffectScope,
    /// All payload members, launchers and helpers were covered.
    pub whole_domain: bool,
    /// Closure remains effective against automatic restart.
    pub restart_prevented: bool,
    /// The selected boundary excludes resurrection from a snapshot.
    pub snapshot_safe: bool,
    /// Independent observation of no remaining members, not a stop request.
    pub quiescent: bool,
    /// Only these reusable dimensions are covered by the proof.
    pub released: Vec<Resource>,
    /// Final measured usage per selected cumulative dimension, possibly absent.
    pub consumption: Vec<ConsumptionObservation>,
    /// Exact closure observation reference, retained without relabeling history.
    pub evidence: SourceReference,
}
/// Canonical version-four inputs; every reducer operation is non-effectful.
#[derive(Clone, PartialEq, Eq)]
pub enum DomainEvent {
    /// One immutable pool registration, never an alias or a budget reset.
    ConfigurePool(PoolContract),
    /// Atomic domain intent and global pool reservation.
    Reserve {
        /// Complete original contract.
        intent: DomainIntent,
        /// Monotonic tick.
        at: u64,
    },
    /// Durable uncertainty before one creation call.
    CreateRequested {
        /// Exact domain.
        domain: DomainId,
        /// Monotonic tick.
        at: u64,
    },
    /// Bind actual incarnation before any arbitrary execution/context release.
    Bound {
        /// Exact domain.
        domain: DomainId,
        /// Actual identity.
        identity: DomainIdentity,
        /// Monotonic tick.
        at: u64,
    },
    /// Bind a complete actual observation before returning a retained channel.
    /// Encoded separately from the legacy identity-only binding event.
    BoundObserved {
        /// Exact domain.
        domain: DomainId,
        /// Actual identity, membership and control observations.
        observation: DomainBindingObservation,
        /// Monotonic tick.
        at: u64,
    },
    /// Durable intent before one payload release; not a claim that it ran.
    ReleaseRequested {
        /// Exact domain.
        domain: DomainId,
        /// Exact bound identity.
        identity: DomainIdentity,
        /// Monotonic tick.
        at: u64,
    },
    /// Retain uncertainty without changing occupancy, usage or original captures.
    Unknown {
        /// Exact domain.
        domain: DomainId,
        /// Monotonic tick.
        at: u64,
        /// Immutable cause reference.
        basis: SourceReference,
    },
    /// Capture a result separately; this does not settle anything or accept artifacts.
    Report {
        /// Exact domain.
        domain: DomainId,
        /// Exact bound identity.
        identity: DomainIdentity,
        /// Captured result.
        outcome: EffectOutcome,
        /// Monotonic tick.
        at: u64,
        /// Observation reference.
        basis: SourceReference,
    },
    /// Request cancellation only. Late binding/release is refused.
    Cancel {
        /// Exact domain.
        domain: DomainId,
        /// Monotonic tick.
        at: u64,
        /// Decision reference.
        basis: SourceReference,
    },
    /// Retain one partial post-cancellation observation without changing holds.
    ClosureObserved {
        /// Exact previously bound domain.
        domain: DomainId,
        /// Bounded native readback, never a closure receipt.
        observation: DomainClosureObservation,
        /// Must equal the unchanged observation tick.
        at: u64,
    },
    /// Settle covered present capacity without changing historical Unknown.
    Closed {
        /// Exact proof and resource coverage.
        receipt: ClosureReceipt,
        /// Monotonic tick.
        at: u64,
    },
    /// Recover exact closure after a lost creation identity receipt. This records
    /// only a closed incarnation and cannot enable binding, payload or adoption.
    RecoveredClosed {
        /// Exact intended request/configuration and observed whole domain.
        receipt: ClosureReceipt,
        /// Monotonic tick.
        at: u64,
    },
    /// Add later exact dimension evidence; does not replace the original closure.
    Settle {
        /// Same exact closed domain and newly supported dimensions.
        receipt: ClosureReceipt,
        /// Monotonic tick.
        at: u64,
    },
    /// Administrative disposition with no technical closure/refund implication.
    Disposition {
        /// Exact domain.
        domain: DomainId,
        /// Administrative selection.
        disposition: AdministrativeDisposition,
        /// Monotonic tick.
        at: u64,
        /// Owner decision reference.
        basis: SourceReference,
    },
}
impl DomainEvent {
    /// Exact affected domain, if this is not pool registration or first reserve.
    pub fn domain(&self) -> Option<&DomainId> {
        match self {
            Self::ConfigurePool(_) => None,
            Self::Reserve { intent, .. } => Some(&intent.id),
            Self::Closed { receipt, .. }
            | Self::RecoveredClosed { receipt, .. }
            | Self::Settle { receipt, .. } => Some(&receipt.domain),
            Self::CreateRequested { domain, .. }
            | Self::Bound { domain, .. }
            | Self::BoundObserved { domain, .. }
            | Self::ClosureObserved { domain, .. }
            | Self::ReleaseRequested { domain, .. }
            | Self::Unknown { domain, .. }
            | Self::Report { domain, .. }
            | Self::Cancel { domain, .. }
            | Self::Disposition { domain, .. } => Some(domain),
        }
    }
    /// Caller-supplied logical tick; replay never reads the clock.
    pub fn tick(&self) -> Option<u64> {
        match self {
            Self::ConfigurePool(_) => None,
            Self::Reserve { at, .. }
            | Self::CreateRequested { at, .. }
            | Self::Bound { at, .. }
            | Self::BoundObserved { at, .. }
            | Self::ClosureObserved { at, .. }
            | Self::ReleaseRequested { at, .. }
            | Self::Unknown { at, .. }
            | Self::Report { at, .. }
            | Self::Cancel { at, .. }
            | Self::Closed { at, .. }
            | Self::RecoveredClosed { at, .. }
            | Self::Settle { at, .. }
            | Self::Disposition { at, .. } => Some(*at),
        }
    }
    /// Whether admission needs fresh operational authority and package validity.
    pub fn needs_current_authority(&self) -> bool {
        matches!(
            self,
            Self::Reserve { .. }
                | Self::CreateRequested { .. }
                | Self::Bound { .. }
                | Self::BoundObserved { .. }
                | Self::ReleaseRequested { .. }
        )
    }
}
/// Reconstructed independent facts; immutable events retain the original basis.
#[derive(Clone, PartialEq, Eq)]
pub struct DomainRecord {
    /// Immutable intention.
    pub intent: DomainIntent,
    /// Present lifecycle/capability evidence.
    pub phase: DomainPhase,
    /// Original historical result, never fabricated by settlement.
    pub outcome: HistoricalOutcome,
    /// Exact captured incarnation, if any.
    pub identity: Option<DomainIdentity>,
    /// Post-create facts, absent for legacy identity-only bindings.
    pub binding_observation: Option<DomainBindingObservation>,
    /// Cancellation request retained independently from closure.
    pub cancelled: bool,
    /// Original durable cancellation tick, reconstructed from the same event.
    pub cancelled_at: Option<u64>,
    /// Immutable bounded partial readbacks, separate from accepted closure.
    pub closure_observations: Vec<DomainClosureObservation>,
    /// Explicit administrative decision only.
    pub disposition: AdministrativeDisposition,
    /// Applicable final closure and dimension-specific disposition, if present.
    pub closure: Option<ClosureReceipt>,
    /// Later immutable, dimension-specific settlement receipts.
    pub settlements: Vec<ClosureReceipt>,
}
impl DomainRecord {
    /// Explain what the native readback boundary has not established. This does
    /// not authenticate captures or override a separate closure receipt.
    pub fn closure_inspection(&self) -> DomainClosureInspection {
        let mut missing = Vec::new();
        if self.binding_observation.is_none() {
            missing.push(ClosureProofGap::BindingObservation);
        }
        if !self.cancelled {
            missing.push(ClosureProofGap::DurableCancellation);
        }
        match self.closure_observations.last() {
            None => missing.push(ClosureProofGap::ClosureObservation),
            Some(o) => {
                if o.populated {
                    missing.push(ClosureProofGap::PopulatedDomain);
                }
                if o.observed_main.is_some() || !o.direct_members.is_empty() {
                    missing.push(ClosureProofGap::RemainingMembers);
                }
                if !o.restart_prevented {
                    missing.push(ClosureProofGap::RestartPrevention);
                }
            }
        }
        missing.extend([
            ClosureProofGap::WholeDomainCoverage,
            ClosureProofGap::HelperCoverage,
            ClosureProofGap::StreamCoverage,
            ClosureProofGap::SnapshotExclusion,
            ClosureProofGap::FinalConsumption,
        ]);
        DomainClosureInspection {
            retained_observations: self.closure_observations.len(),
            receipt_recorded: self.closure.is_some(),
            missing,
        }
    }

    /// Whether this reusable dimension is still held.
    pub fn holds(&self, resource: Resource) -> bool {
        self.phase != DomainPhase::CancelledBeforeCreate
            && self
                .intent
                .effect
                .demand
                .iter()
                .any(|d| d.resource == resource)
            && !self
                .closure
                .iter()
                .chain(self.settlements.iter())
                .any(|c| c.released.contains(&resource))
    }
    /// Complete retained measurement, if present; absent is never inferred as zero.
    pub fn measured(&self, dimension: Consumption) -> Option<u64> {
        self.closure
            .iter()
            .chain(self.settlements.iter())
            .flat_map(|c| c.consumption.iter())
            .find(|c| c.dimension == dimension && c.measured.is_some())
            .and_then(|c| c.measured)
    }
    /// Inspect complete cumulative measurement across immutable settlement receipts.
    pub fn measured_consumption(&self, dimension: Consumption) -> Option<u64> {
        self.measured(dimension)
    }
    /// Missing final measured use after creation is debt with no proven upper bound.
    pub fn unresolved_consumption(&self, dimension: Consumption) -> bool {
        !matches!(
            self.phase,
            DomainPhase::Reserved | DomainPhase::CancelledBeforeCreate
        ) && self.measured(dimension).is_none()
    }
}
/// Deterministic refusal; errors never expose private paths or raw manager output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainError {
    /// Malformed/boundedness violation.
    Structure,
    /// Immutable identity conflicts or is absent.
    Correspondence,
    /// Unsupported lifecycle transition.
    Transition,
    /// Resource capacity exhausted.
    Capacity(Resource),
    /// Mandatory cumulative quantity is unresolved, not zero.
    Debt(Consumption),
    /// Required cumulative allowance exceeded.
    Budget(Consumption),
    /// Caller clock regressed.
    ClockRegression,
    /// Maximum retained domains exceeded.
    Full,
}
impl std::fmt::Display for DomainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "execution domain: {self:?}")
    }
}
impl std::error::Error for DomainError {}
/// Pure Core projection, admitted exclusively through the canonical Workflow journal.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct DomainProjection {
    pool: Option<PoolContract>,
    domains: Vec<DomainRecord>,
    events: Vec<DomainEvent>,
    last_tick: u64,
}
impl DomainProjection {
    /// Selected immutable physical pool.
    pub fn pool(&self) -> Option<&PoolContract> {
        self.pool.as_ref()
    }
    /// Retained domains across all registered programs.
    pub fn domains(&self) -> &[DomainRecord] {
        &self.domains
    }
    /// Exact immutable domain observations.
    pub fn events(&self) -> &[DomainEvent] {
        &self.events
    }
    /// Last caller-supplied logical tick.
    pub fn last_tick(&self) -> u64 {
        self.last_tick
    }
    /// Find one exact retained domain, without host I/O.
    pub fn domain(&self, id: &DomainId) -> Option<&DomainRecord> {
        self.domains.iter().find(|d| &d.intent.id == id)
    }
    /// Sum held occupancy across every program in the same physical pool.
    pub fn held(&self, resource: Resource) -> Result<u64, DomainError> {
        self.domains
            .iter()
            .filter(|d| d.holds(resource))
            .try_fold(0u64, |n, d| {
                n.checked_add(
                    d.intent
                        .effect
                        .demand
                        .iter()
                        .find(|v| v.resource == resource)
                        .and_then(|v| v.amount)
                        .ok_or(DomainError::Structure)?,
                )
                .ok_or(DomainError::Capacity(resource))
            })
    }
    /// Account known use and future reservations; an unknown post-create debit blocks.
    pub fn consumed_or_reserved(&self, dimension: Consumption) -> Result<u64, DomainError> {
        self.domains.iter().try_fold(0u64, |n, d| {
            if d.phase == DomainPhase::CancelledBeforeCreate {
                return Ok(n);
            }
            let amount = if d.phase == DomainPhase::Reserved {
                d.intent
                    .consumption
                    .iter()
                    .find(|x| x.dimension == dimension)
                    .map(|x| x.amount)
                    .ok_or(DomainError::Structure)?
            } else {
                d.measured(dimension).ok_or(DomainError::Debt(dimension))?
            };
            n.checked_add(amount).ok_or(DomainError::Budget(dimension))
        })
    }
    /// Structurally apply one event atomically. The composing Evidence kernel MUST
    /// also validate member authority, exact package, review and current dependency state.
    pub fn apply(&mut self, event: DomainEvent) -> Result<(), DomainError> {
        let mut next = self.clone();
        if let Some(at) = event.tick() {
            if at < next.last_tick {
                return Err(DomainError::ClockRegression);
            }
            next.last_tick = at;
        }
        next.reduce(&event)?;
        next.events.push(event);
        *self = next;
        Ok(())
    }
    fn reduce(&mut self, event: &DomainEvent) -> Result<(), DomainError> {
        match event {
            DomainEvent::ConfigurePool(pool) => {
                if self.pool.is_some() {
                    return Err(DomainError::Correspondence);
                }
                pool.identity.validate()?;
                if pool.max_domains == 0
                    || pool.max_domains > 4096
                    || pool.capacities.is_empty()
                    || pool.capacities.len() > 6
                    || pool.consumption.is_empty()
                    || pool.consumption.len() > 5
                {
                    return Err(DomainError::Structure);
                }
                let mut seen = BTreeSet::new();
                for c in &pool.capacities {
                    if !reusable(c.resource)
                        || !seen.insert(c.resource)
                        || !c.declared_enforceable
                        || c.available.is_none_or(|n| n <= c.verification_reserve)
                    {
                        return Err(DomainError::Structure);
                    }
                }
                let mut seen = BTreeSet::new();
                for c in &pool.consumption {
                    if !seen.insert(c.dimension)
                        || c.per_operation == 0
                        || c.total
                            .checked_sub(c.reserve)
                            .is_none_or(|n| n < c.per_operation)
                    {
                        return Err(DomainError::Structure);
                    }
                }
                self.pool = Some(pool.clone());
            }
            DomainEvent::Reserve { intent, .. } => {
                let pool = self.pool.as_ref().ok_or(DomainError::Structure)?;
                if intent.pool != pool.identity
                    || intent.effect.retry_of.is_some()
                    || self.domains.iter().any(|d| {
                        d.intent.id == intent.id
                            || d.intent.request == intent.request
                            || (d.intent.effect.scope.program == intent.effect.scope.program
                                && (d.intent.effect.id == intent.effect.id
                                    || d.intent.effect.scope.attempt
                                        == intent.effect.scope.attempt))
                    })
                {
                    return Err(DomainError::Correspondence);
                }
                if self.domains.len() >= pool.max_domains {
                    return Err(DomainError::Full);
                }
                let mut seen = BTreeSet::new();
                for d in &intent.effect.demand {
                    if !seen.insert(d.resource) || d.amount.is_none_or(|n| n == 0) {
                        return Err(DomainError::Structure);
                    }
                }
                for cap in &pool.capacities {
                    let requested = intent
                        .effect
                        .demand
                        .iter()
                        .find(|d| d.resource == cap.resource)
                        .and_then(|d| d.amount)
                        .ok_or(DomainError::Capacity(cap.resource))?;
                    if self
                        .held(cap.resource)?
                        .checked_add(requested)
                        .is_none_or(|n| n > cap.available.unwrap_or(0) - cap.verification_reserve)
                    {
                        return Err(DomainError::Capacity(cap.resource));
                    }
                }
                if intent.effect.demand.iter().any(|d| {
                    reusable(d.resource)
                        && !pool.capacities.iter().any(|c| c.resource == d.resource)
                }) {
                    return Err(DomainError::Structure);
                }
                if intent.consumption.len() != pool.consumption.len() {
                    return Err(DomainError::Structure);
                }
                let mut seen = BTreeSet::new();
                for debit in &intent.consumption {
                    let budget = pool
                        .consumption
                        .iter()
                        .find(|b| b.dimension == debit.dimension)
                        .ok_or(DomainError::Structure)?;
                    if !seen.insert(debit.dimension)
                        || debit.amount == 0
                        || debit.amount > budget.per_operation
                    {
                        return Err(DomainError::Budget(debit.dimension));
                    }
                    if self
                        .consumed_or_reserved(debit.dimension)?
                        .checked_add(debit.amount)
                        .is_none_or(|n| n > budget.total - budget.reserve)
                    {
                        return Err(DomainError::Budget(debit.dimension));
                    }
                }
                self.domains.push(DomainRecord {
                    intent: intent.clone(),
                    phase: DomainPhase::Reserved,
                    outcome: HistoricalOutcome::NotDispatched,
                    identity: None,
                    binding_observation: None,
                    cancelled: false,
                    cancelled_at: None,
                    closure_observations: vec![],
                    disposition: AdministrativeDisposition::Active,
                    closure: None,
                    settlements: vec![],
                });
            }
            _ => {
                let id = event.domain().ok_or(DomainError::Structure)?;
                let record = self
                    .domains
                    .iter_mut()
                    .find(|d| &d.intent.id == id)
                    .ok_or(DomainError::Correspondence)?;
                if event.needs_current_authority()
                    && (record.cancelled || record.disposition != AdministrativeDisposition::Active)
                {
                    return Err(DomainError::Transition);
                }
                match event {
                    DomainEvent::CreateRequested { .. } => {
                        if record.phase != DomainPhase::Reserved {
                            return Err(DomainError::Transition);
                        }
                        record.phase = DomainPhase::CreateUncertain;
                        record.outcome = HistoricalOutcome::Unknown;
                    }
                    DomainEvent::Bound { identity, .. } => {
                        identity.validate()?;
                        if record.phase != DomainPhase::CreateUncertain
                            || identity.pool != record.intent.pool
                            || identity.request != record.intent.request
                        {
                            return Err(DomainError::Correspondence);
                        }
                        record.identity = Some(identity.clone());
                        record.phase = DomainPhase::Bound;
                    }
                    DomainEvent::BoundObserved { observation, .. } => {
                        observation.validate()?;
                        if record.phase != DomainPhase::CreateUncertain
                            || observation.identity.pool != record.intent.pool
                            || observation.identity.request != record.intent.request
                            || observation.configuration != record.intent.configuration
                        {
                            return Err(DomainError::Correspondence);
                        }
                        record.identity = Some(observation.identity.clone());
                        record.binding_observation = Some(observation.clone());
                        record.phase = DomainPhase::Bound;
                    }
                    DomainEvent::ReleaseRequested { identity, .. } => {
                        if record.phase != DomainPhase::Bound
                            || record.identity.as_ref() != Some(identity)
                        {
                            return Err(DomainError::Correspondence);
                        }
                        record.phase = DomainPhase::ReleaseUncertain;
                    }
                    DomainEvent::Unknown { .. } => {
                        if matches!(
                            record.phase,
                            DomainPhase::Reserved
                                | DomainPhase::Closed
                                | DomainPhase::CancelledBeforeCreate
                        ) {
                            return Err(DomainError::Transition);
                        }
                        if matches!(record.outcome, HistoricalOutcome::Reported(_)) {
                            return Err(DomainError::Transition);
                        }
                        record.outcome = HistoricalOutcome::Unknown;
                    }
                    DomainEvent::Report {
                        identity, outcome, ..
                    } => {
                        if record.phase != DomainPhase::ReleaseUncertain
                            || record.identity.as_ref() != Some(identity)
                            || matches!(record.outcome, HistoricalOutcome::Reported(_))
                        {
                            return Err(DomainError::Transition);
                        }
                        record.outcome = HistoricalOutcome::Reported(*outcome);
                    }
                    DomainEvent::Cancel { at, .. } => {
                        if record.cancelled || record.phase == DomainPhase::Closed {
                            return Err(DomainError::Transition);
                        }
                        record.cancelled = true;
                        record.cancelled_at = Some(*at);
                        if record.phase == DomainPhase::Reserved {
                            record.phase = DomainPhase::CancelledBeforeCreate;
                        }
                    }
                    DomainEvent::ClosureObserved {
                        observation, at, ..
                    } => {
                        observation.validate()?;
                        let binding = record
                            .binding_observation
                            .as_ref()
                            .ok_or(DomainError::Correspondence)?;
                        if !record.cancelled
                            || record.closure.is_some()
                            || observation.observed_at != *at
                            || record.cancelled_at.is_none_or(|cancel| *at < cancel)
                            || record.identity.as_ref() != Some(&observation.identity)
                            || observation.configuration != record.intent.configuration
                            || observation.scope != record.intent.effect.scope
                            || observation.binding_main != binding.main
                            || record
                                .closure_observations
                                .last()
                                .is_some_and(|old| old.observed_at >= *at)
                        {
                            return Err(DomainError::Correspondence);
                        }
                        if record.closure_observations.len() >= 32 {
                            return Err(DomainError::Full);
                        }
                        for member in &observation.direct_members {
                            if binding
                                .direct_members
                                .iter()
                                .any(|old| old.pid == member.pid && old != member)
                            {
                                return Err(DomainError::Correspondence);
                            }
                        }
                        record.closure_observations.push(observation.clone());
                    }
                    DomainEvent::Closed { receipt, .. }
                    | DomainEvent::RecoveredClosed { receipt, .. }
                    | DomainEvent::Settle { receipt, .. } => {
                        receipt.identity.validate()?;
                        let settling = matches!(event, DomainEvent::Settle { .. });
                        let recovered = matches!(event, DomainEvent::RecoveredClosed { .. });
                        let identity_matches = if recovered {
                            record.identity.is_none()
                                && record.phase == DomainPhase::CreateUncertain
                                && receipt.identity.pool == record.intent.pool
                                && receipt.identity.request == record.intent.request
                        } else {
                            record.identity.as_ref() == Some(&receipt.identity)
                        };
                        if matches!(
                            record.phase,
                            DomainPhase::Reserved | DomainPhase::CancelledBeforeCreate
                        ) || (record.phase == DomainPhase::Closed) != settling
                            || !identity_matches
                            || receipt.configuration != record.intent.configuration
                            || receipt.scope != record.intent.effect.scope
                            || !receipt.whole_domain
                            || !receipt.restart_prevented
                            || !receipt.snapshot_safe
                            || !receipt.quiescent
                        {
                            return Err(DomainError::Correspondence);
                        }
                        let mut seen = BTreeSet::new();
                        for resource in &receipt.released {
                            if !record.holds(*resource)
                                || !reusable(*resource)
                                || !seen.insert(*resource)
                                || !record
                                    .intent
                                    .effect
                                    .demand
                                    .iter()
                                    .any(|d| d.resource == *resource)
                            {
                                return Err(DomainError::Structure);
                            }
                        }
                        let mut seen = BTreeSet::new();
                        for usage in &receipt.consumption {
                            if record.measured(usage.dimension).is_some()
                                || !seen.insert(usage.dimension)
                                || !record
                                    .intent
                                    .consumption
                                    .iter()
                                    .any(|d| d.dimension == usage.dimension)
                            {
                                return Err(DomainError::Structure);
                            }
                        }
                        if settling {
                            if receipt.released.is_empty()
                                && !receipt.consumption.iter().any(|c| c.measured.is_some())
                            {
                                return Err(DomainError::Structure);
                            }
                            record.settlements.push(receipt.clone());
                        } else {
                            if recovered {
                                record.identity = Some(receipt.identity.clone());
                            }
                            record.closure = Some(receipt.clone());
                            record.phase = DomainPhase::Closed;
                        }
                    }
                    DomainEvent::Disposition { disposition, .. } => {
                        if record.disposition == *disposition
                            || record.disposition == AdministrativeDisposition::Abandoned
                        {
                            return Err(DomainError::Transition);
                        }
                        record.disposition = *disposition;
                    }
                    _ => return Err(DomainError::Structure),
                }
            }
        }
        Ok(())
    }
}
fn reusable(resource: Resource) -> bool {
    matches!(
        resource,
        Resource::Workers
            | Resource::Builds
            | Resource::Tests
            | Resource::CpuMillis
            | Resource::RamBytes
            | Resource::DiskBytes
    )
}
fn text(value: &str) -> Result<(), DomainError> {
    if value.is_empty() || value.len() > 1024 || value.chars().any(char::is_control) {
        Err(DomainError::Structure)
    } else {
        Ok(())
    }
}
