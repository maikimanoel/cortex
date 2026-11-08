//! Controlled and explicitly supervised fixture contracts with one projection.
//!
//! These records model intent, uncertainty and resource accounting without I/O.
//! Supervised fixtures additionally require the selected durable journal and host
//! boundary. Authority, time and enforcement references remain caller assertions;
//! no general executor permission follows. Workflow owns retention and replay.

mod recovery;
pub use recovery::*;

use super::supervision::*;
use super::{
    scheduling::{Capacity, Demand, Resource},
    *,
};
use crate::binding::CandidateRequirement;

/// Distinct immutable effect identity, separate from the journal event identity.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct EffectId(String);
impl EffectId {
    /// Validate bounded identity text, without authenticating its source.
    pub fn new(value: &str) -> Result<Self, ProgramError> {
        crate::validate_identifier("effect_id", value)?;
        Ok(Self(value.to_owned()))
    }
    /// Explicit caller-supplied identity text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl fmt::Debug for EffectId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("EffectId([redacted])")
    }
}
/// Explicit fixture profiles; declaring a real external executor still fails.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EffectMode {
    /// No real executor or external operation is represented by this profile.
    #[default]
    ControlledFixture,
    /// Explicit trusted, provider-free owned development process profile.
    SupervisedFixture,
    /// Reserved refusal case; no implementation or authority is supplied.
    ExternalExecutor,
}
/// Explicit immutable fixture policy; no budget or privacy defaults are selected.
#[derive(Clone, PartialEq, Eq)]
pub struct Policy {
    /// Exact immutable policy identity and revision.
    pub reference: SourceReference,
    /// Accepted program identity.
    pub program: ProgramId,
    /// Exact protected baseline.
    pub baseline: ContractRevision,
    /// Fixture-only support boundary.
    pub mode: EffectMode,
    /// Caller-supplied host capability reference, not host qualification.
    pub host: SourceReference,
    /// Caller-supplied disclosure/retention policy reference.
    pub privacy: SourceReference,
    /// Explicit independent capacities and reserved verification quantities.
    pub capacities: Vec<Capacity>,
    /// Dimensions consumed across settled effects instead of released on completion.
    pub cumulative: Vec<Resource>,
    /// Maximum total retained effects across policies, positive and program-bounded.
    pub max_effects: usize,
    /// Maximum additional attempts in a proven-absent retry chain; zero disables retry.
    pub max_retries: u32,
}
/// Exact task, candidate and ownership correspondence of an effect.
#[derive(Clone, PartialEq, Eq)]
pub struct EffectScope {
    /// Accepted program identity.
    pub program: ProgramId,
    /// Exact protected baseline.
    pub baseline: ContractRevision,
    /// Target task.
    pub task: TaskId,
    /// Full repository/worktree/commit requirement.
    pub candidate: CandidateRequirement,
    /// Current metadata ownership attempt.
    pub attempt: AttemptId,
    /// Exact task generation.
    pub generation: ContractRevision,
    /// Stored instruction package; the composing consumer checks its actual bytes.
    pub package: PackageId,
    /// Exact stored package revision.
    pub package_revision: ContractRevision,
}
/// Explicit local grant assertion checked within the selected fixture profile.
#[derive(Clone, PartialEq, Eq)]
pub struct OperationalGrant {
    /// Immutable grant identity; changed reuse fails.
    pub id: GrantId,
    /// Claimed principal; must match the metadata attempt owner.
    pub principal: ActorId,
    /// Exact immutable policy.
    pub policy: SourceReference,
    /// Exact protected operation scope.
    pub scope: EffectScope,
    /// Inclusive caller-provided logical tick; no clock authenticity is implied.
    pub not_before: u64,
    /// Exclusive caller-provided logical expiry, strictly later than not_before.
    pub expires_before: u64,
    /// Per-effect ceilings for every policy dimension, with explicit known quantities.
    pub ceilings: Vec<Demand>,
    /// Explicit approval reference, not authenticated permission.
    pub basis: SourceReference,
}
/// Intent and resource reservation admitted atomically in the fixture projection.
#[derive(Clone, PartialEq, Eq)]
pub struct EffectIntent {
    /// Unique immutable effect identity.
    pub id: EffectId,
    /// Exact retained proposed grant.
    pub grant: GrantId,
    /// Exact scope repeated to reject ambiguity and changed correspondence.
    pub scope: EffectScope,
    /// Explicit positive demand for every policy dimension.
    pub demand: Vec<Demand>,
    /// Explicit prior effect for a bounded retry after proven absence only.
    pub retry_of: Option<EffectId>,
}
/// Observed fixture result; success is not review, acceptance or integration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectOutcome {
    /// Fixture reported its scoped action succeeded.
    Succeeded,
    /// Fixture reported failure; the original result is retained.
    Failed,
    /// Fixture reported interruption with no success inference.
    Interrupted,
}
/// Distinct lifecycle states; none confers authority or task completion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectStatus {
    /// Intent and simulated quantities retained; dispatch has not been marked.
    Reserved,
    /// Dispatch boundary may have been crossed; absence cannot be inferred.
    DispatchUncertain,
    /// A fixture reported running; absence is no longer a valid reconciliation.
    Running,
    /// Available observations cannot determine the outcome.
    Unknown,
    /// Cancellation requested; no cleanup or termination is inferred.
    CancellationRequested,
    /// A report is retained; resources await explicit reconciliation.
    Reported,
    /// Pre-dispatch cancellation, with no attempted dispatch.
    CancelledBeforeDispatch,
    /// Explicit exact reconciliation established no effect occurred.
    ReconciledAbsent,
    /// Explicit exact reconciliation settled the reported/observed result and usage.
    ReconciledReported,
}
/// Explicit reconciliation claim, never inferred from silence or a timeout.
#[derive(Clone, PartialEq, Eq)]
pub enum Reconciliation {
    /// Exact fixture inspection establishes no dispatch/effect occurred.
    Absent,
    /// A terminal result with complete fixture usage; supervised measured overruns
    /// remain charged when explicitly settling a quiescent failure/interruption.
    Observed {
        /// Result being reconciled; cannot rewrite a retained contradictory report.
        outcome: EffectOutcome,
        /// Known nonnegative usage for every dimension. Pure fixture usage is
        /// bounded by reservation; supervised usage must match captured quantities.
        usage: Vec<Demand>,
    },
}
/// Complete retained input. Journal event identity/replay is owned by Workflow.
#[derive(Clone, PartialEq, Eq)]
pub enum EffectEvent {
    /// Install a proposed immutable policy; selects the current fixture policy.
    Policy(Policy),
    /// Retain an exact proposed grant, without authorizing a real operation.
    Grant(OperationalGrant),
    /// Revoke a proposed grant; existing uncertain effects retain reservations.
    Revoke {
        /// Exact retained grant identity.
        grant: GrantId,
        /// Monotonic caller-supplied logical tick.
        at: u64,
        /// Explicit revocation decision reference.
        basis: SourceReference,
    },
    /// Atomically retain intent and proposed resources under current contracts.
    Reserve {
        /// Complete exact input to admission.
        intent: EffectIntent,
        /// Monotonic caller-supplied logical tick.
        at: u64,
    },
    /// Record dispatch uncertainty before any fixture interaction; never dispatches.
    Dispatch {
        /// Exact retained effect identity.
        effect: EffectId,
        /// Exact original effect scope.
        scope: EffectScope,
        /// Monotonic caller-supplied logical tick.
        at: u64,
        /// Explicit fixture dispatch-intent reference.
        basis: SourceReference,
    },
    /// Bind one immutable process specification before a supervised dispatch.
    PrepareProcess {
        /// Exact retained effect identity.
        effect: EffectId,
        /// Exact current effect scope.
        scope: EffectScope,
        /// Controller logical millisecond tick.
        at: u64,
        /// Complete finite process specification, never executable instructions.
        spec: Box<ProcessSpec>,
    },
    /// Append a captured process observation; this performs no process operation.
    ProcessObserved {
        /// Exact retained effect identity.
        effect: EffectId,
        /// Exact historical scope, retained even after revocation or amendment.
        scope: EffectScope,
        /// Controller logical millisecond tick.
        at: u64,
        /// Immutable observation, distinct from a consequential assessment.
        observation: ProcessObservation,
    },
    /// Retain fixture running observation without creating completion evidence.
    Running {
        /// Exact retained effect identity.
        effect: EffectId,
        /// Exact original effect scope, still current at observation admission.
        scope: EffectScope,
        /// Monotonic caller-supplied logical tick.
        at: u64,
        /// Caller-reported fixture observation reference.
        basis: SourceReference,
    },
    /// Retain an original fixture report; does not release resources or dependencies.
    Report {
        /// Exact retained effect identity.
        effect: EffectId,
        /// Exact original effect scope, still current at observation admission.
        scope: EffectScope,
        /// Monotonic caller-supplied logical tick.
        at: u64,
        /// Original reported result, distinct from acceptance.
        outcome: EffectOutcome,
        /// Caller-reported result reference.
        basis: SourceReference,
    },
    /// Preserve an uncertain result and all outstanding reservation quantities.
    Unknown {
        /// Exact retained effect identity.
        effect: EffectId,
        /// Exact historical effect scope.
        scope: EffectScope,
        /// Monotonic caller-supplied logical tick.
        at: u64,
        /// Explicit interruption or uncertainty observation reference.
        basis: SourceReference,
    },
    /// Request cancellation; after dispatch this cannot assert process termination.
    Cancel {
        /// Exact retained effect identity.
        effect: EffectId,
        /// Exact historical effect scope.
        scope: EffectScope,
        /// Monotonic caller-supplied logical tick.
        at: u64,
        /// Explicit cancellation request reference.
        basis: SourceReference,
    },
    /// Reconcile exact historical scope, including revoked or superseded baselines.
    Reconcile {
        /// Exact retained effect identity.
        effect: EffectId,
        /// Exact historical effect scope.
        scope: EffectScope,
        /// Monotonic caller-supplied logical tick.
        at: u64,
        /// Explicit supplied resolution, never inferred from silence.
        resolution: Reconciliation,
        /// Caller-reported exact fixture reconciliation reference.
        basis: SourceReference,
    },
}
/// One immutable effect's reconstructed disposition and retained original inputs.
#[derive(Clone, PartialEq, Eq)]
pub struct EffectRecord {
    /// Complete original intent.
    pub intent: EffectIntent,
    /// Exact original policy reference.
    pub policy: SourceReference,
    /// Current fixture-only disposition.
    pub status: EffectStatus,
    /// Original report; reconciliation cannot replace it.
    pub report: Option<EffectOutcome>,
    /// Complete final reconciliation, when present.
    pub reconciliation: Option<Reconciliation>,
    /// A positive running observation prevents later absence claims.
    pub observed_running: bool,
    /// Whether cancellation was requested at any point; retained through unknown
    /// outcomes, subsequent reports and reconciliation, without implying cleanup.
    pub cancellation_requested: bool,
    /// Number of prior effects in this retry chain.
    pub retries: u32,
    /// Exact process contract, present only in the explicitly supervised profile.
    pub process_spec: Option<ProcessSpec>,
    /// Original captured process observations; none is overwritten by recovery.
    pub process_observations: Vec<ProcessObservation>,
    /// Durable prelaunch tick; a fresh controller may map it to elapsed milliseconds.
    /// Replay cannot create a new clock origin or reattach to a retained PID.
    pub dispatch_at: Option<u64>,
}
/// Typed, private-input-safe refusal from the deterministic fixture model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperationalError {
    /// Existing Core contract or correspondence refusal.
    Program(ProgramError),
    /// A policy declares an unavailable real execution mode.
    ExternalUnavailable,
    /// The required resource has no explicit quantity.
    UnknownResource(Resource),
    /// The required resource lacks even a declared fixture enforcement mechanism.
    UnenforceableResource(Resource),
    /// Required quantity does not fit remaining capacity or the grant ceiling.
    ResourceLimit(Resource),
    /// A proposed grant expired, was revoked, or is not yet applicable.
    GrantUnavailable,
    /// A caller-provided time moved backward relative to admitted events.
    ClockRegression,
}
impl From<ProgramError> for OperationalError {
    fn from(e: ProgramError) -> Self {
        Self::Program(e)
    }
}
impl From<ReadinessError> for OperationalError {
    fn from(e: ReadinessError) -> Self {
        Self::Program(e.into())
    }
}
impl fmt::Display for OperationalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Program(e) => e.fmt(f),
            other => write!(f, "operational fixture: {other:?}"),
        }
    }
}
impl Error for OperationalError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Program(e) => Some(e),
            _ => None,
        }
    }
}
/// Rebuildable fixture projection; never an operational authority or effect store.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct OperationalProjection {
    mode: EffectMode,
    policies: Vec<Policy>,
    grants: Vec<OperationalGrant>,
    revoked: Vec<GrantId>,
    effects: Vec<EffectRecord>,
    events: Vec<EffectEvent>,
    last_tick: u64,
}

impl OperationalProjection {
    /// Construct an empty projection with no policy, grant or operation authority.
    pub fn new() -> Self {
        Self::default()
    }
    /// Select the explicit trusted development-process profile. This still
    /// performs no I/O and cannot supply a launch permit without its journal owner.
    pub fn new_supervised_fixture() -> Self {
        Self {
            mode: EffectMode::SupervisedFixture,
            ..Self::default()
        }
    }
    /// Exact selected interpretation; it cannot change after construction.
    pub fn mode(&self) -> EffectMode {
        self.mode
    }
    /// Borrow the immutable process specification for an exact retained effect.
    pub fn process_spec(&self, effect: &EffectId) -> Option<&ProcessSpec> {
        self.effects
            .iter()
            .find(|e| &e.intent.id == effect)
            .and_then(|e| e.process_spec.as_ref())
    }
    /// Recheck current scope, policy, revocation and logical millisecond expiry
    /// for an already-dispatched owned fixture. This inspection grants no new
    /// launch, assumes no process cleanup and never creates a restart clock origin.
    pub fn check_supervised_current(
        &self,
        state: &ProgramState,
        effect: &EffectId,
        scope: &EffectScope,
        at: u64,
    ) -> Result<(), OperationalError> {
        if self.mode != EffectMode::SupervisedFixture {
            return Err(OperationalError::ExternalUnavailable);
        }
        if at < self.last_tick {
            return Err(OperationalError::ClockRegression);
        }
        let i = self.effect_index(effect, scope)?;
        let record = &self.effects[i];
        if record.process_spec.is_none()
            || record.dispatch_at.is_none()
            || record.cancellation_requested
            || !matches!(
                record.status,
                EffectStatus::DispatchUncertain | EffectStatus::Running
            )
            || at < record.dispatch_at.ok_or(ProgramError::Missing)?
        {
            return Err(OperationalError::GrantUnavailable);
        }
        self.check_grant(state, self.grant(&record.intent.grant)?, scope, at)
    }
    /// Validate existing grant, claim, candidate and per-operation ceilings for a
    /// version-four domain. The single global pool coordinator owns occupancy;
    /// this check neither reserves resources nor creates an alternate grant.
    pub fn check_domain_intent(
        &self,
        state: &ProgramState,
        intent: &EffectIntent,
        at: u64,
    ) -> Result<(), OperationalError> {
        if self.mode != EffectMode::SupervisedFixture || at < self.last_tick {
            return Err(OperationalError::GrantUnavailable);
        }
        let grant = self.grant(&intent.grant)?;
        self.check_grant(state, grant, &intent.scope, at)?;
        let policy = self.policy(&grant.policy)?;
        validate_quantities(&intent.demand, &policy.capacities, false)?;
        for demand in &intent.demand {
            let cap = policy
                .capacities
                .iter()
                .find(|c| c.resource.clone() == demand.resource.clone())
                .ok_or(ProgramError::Structure)?;
            if !cap.declared_enforceable {
                return Err(OperationalError::UnenforceableResource(
                    demand.resource.clone(),
                ));
            }
            let available = cap
                .available
                .ok_or(OperationalError::UnknownResource(demand.resource.clone()))?
                .checked_sub(cap.verification_reserve)
                .ok_or(OperationalError::ResourceLimit(demand.resource.clone()))?;
            let required = quantity(&intent.demand, demand.resource.clone())?;
            if required > quantity(&grant.ceilings, demand.resource.clone())?
                || required > available
            {
                return Err(OperationalError::ResourceLimit(demand.resource.clone()));
            }
        }
        if intent.retry_of.is_some() {
            return Err(ProgramError::Transition.into());
        }
        Ok(())
    }
    /// Retained exact policies; only the last is current for new reservations.
    pub fn policies(&self) -> &[Policy] {
        &self.policies
    }
    /// Current proposed policy; absence blocks reservations.
    pub fn current_policy(&self) -> Option<&Policy> {
        self.policies.last()
    }
    /// Exact retained proposed grants, including revoked grants.
    pub fn grants(&self) -> &[OperationalGrant] {
        &self.grants
    }
    /// Irreversibly revoked grant identities in this retained history.
    pub fn revoked(&self) -> &[GrantId] {
        &self.revoked
    }
    /// Full retained effects with historical outcomes and reservations.
    pub fn effects(&self) -> &[EffectRecord] {
        &self.effects
    }
    /// Original ordered fixture observations, including failures and unknowns.
    pub fn events(&self) -> &[EffectEvent] {
        &self.events
    }
    /// Last caller-supplied logical tick, not a wall-clock observation.
    pub fn last_tick(&self) -> u64 {
        self.last_tick
    }
    /// A pending fixture effect blocks responsibility handoff/release by its consumer.
    pub fn has_pending_attempt(&self, attempt: &AttemptId) -> bool {
        self.effects
            .iter()
            .any(|e| &e.intent.scope.attempt == attempt && e.pending())
    }
    /// Accounted units across every retained policy: unsettled reservations plus
    /// settled usage of cumulative dimensions. A new grant/policy does not reset it.
    pub fn accounted(&self, resource: Resource) -> Result<u64, OperationalError> {
        self.effects.iter().try_fold(0_u64, |sum, e| {
            let quantity = if e.pending() {
                let reserved = quantity(&e.intent.demand, resource.clone())?;
                if matches!(resource, Resource::MemoryBytes(_))
                    && self.mode == EffectMode::SupervisedFixture
                    && !matches!(e.status, EffectStatus::Reserved)
                    && e.intent.demand.iter().any(|d| d.resource == resource)
                {
                    // The legacy scope capture has no retained host/boot memory
                    // correspondence. A declared allocation cannot fill that gap.
                    // Keep the reservation, but refuse a fabricated measured debit.
                    return Err(OperationalError::UnknownResource(resource.clone()));
                }
                let observed = if self.mode == EffectMode::SupervisedFixture {
                    match e.process_observations.last() {
                        Some(o) if resource == Resource::ElapsedMillis => o.elapsed_ms,
                        Some(o) if resource == Resource::OutputBytes => o
                            .stdout_bytes
                            .checked_add(o.stderr_bytes)
                            .ok_or(ProgramError::Capacity)?,
                        Some(o)
                            if e.process_spec
                                .as_ref()
                                .and_then(|s| s.containment.as_ref())
                                .is_some_and(|s| s.memory_dimension() == resource) =>
                        {
                            o.containment
                                .as_ref()
                                .and_then(|c| c.memory_peak_bytes)
                                .unwrap_or(0)
                        }
                        _ => 0,
                    }
                } else {
                    0
                };
                // An overrun is an observation, not permission to spend more.
                // Preserve the known debit instead of pretending a reservation capped it.
                reserved.max(observed)
            } else {
                match &e.reconciliation {
                    Some(Reconciliation::Observed { usage, .. })
                        if self.policy(&e.policy)?.cumulative.contains(&resource) =>
                    {
                        quantity(usage, resource.clone())?
                    }
                    Some(Reconciliation::Absent)
                        if self.mode == EffectMode::SupervisedFixture
                            && resource == Resource::ElapsedMillis
                            && self.policy(&e.policy)?.cumulative.contains(&resource) =>
                    {
                        // A measured pre-spawn refusal consumes controller time,
                        // despite proving that no fixture child was started.
                        e.process_observations
                            .last()
                            .ok_or(ProgramError::Missing)?
                            .elapsed_ms
                    }
                    _ => 0,
                }
            };
            sum.checked_add(quantity)
                .ok_or(OperationalError::ResourceLimit(resource.clone()))
        })
    }
    /// Preview the existing explainable scheduler against this ledger's remaining
    /// quantities. Pending unknown effects retain reservations and settled cumulative
    /// usage remains charged. The current policy supplies every required dimension;
    /// missing demands are malformed, while unknown quantities remain held proposals.
    /// This inspection creates no reservation, grant, dispatch or policy change.
    pub fn preview_schedule(
        &self,
        state: &ProgramState,
        proposals: &[super::scheduling::Proposal],
    ) -> Result<Vec<super::scheduling::SchedulingDecision>, OperationalError> {
        let policy = self.current_policy().ok_or(ProgramError::Missing)?;
        if policy.program != *state.definition().id()
            || policy.baseline != state.definition().revision()
        {
            return Err(ProgramError::Stale.into());
        }
        if proposals.len() > state.definition().limits().entries {
            return Err(ProgramError::Capacity.into());
        }
        let required = resource_set(&policy.capacities);
        for proposal in proposals {
            if proposal.demand.len() != policy.capacities.len()
                || proposal
                    .demand
                    .iter()
                    .map(|d| d.resource.clone())
                    .collect::<BTreeSet<_>>()
                    != required
            {
                return Err(ProgramError::Structure.into());
            }
            if let Some(zero) = proposal.demand.iter().find(|d| d.amount == Some(0)) {
                return Err(OperationalError::ResourceLimit(zero.resource.clone()));
            }
        }
        let capacities = policy
            .capacities
            .iter()
            .map(|capacity| {
                let mut remaining = capacity.clone();
                let accounted = self.accounted(capacity.resource.clone())?;
                remaining.available = capacity.available.map(|n| n.saturating_sub(accounted));
                Ok(remaining)
            })
            .collect::<Result<Vec<_>, OperationalError>>()?;
        Ok(super::scheduling::simulate(state, proposals, &capacities)?)
    }
    /// Atomically apply one explicit fixture event. This function has no I/O,
    /// executor or process-control path. The composing journal
    /// handles immutable event IDs and exact replay before calling this reducer.
    pub fn apply(
        &mut self,
        state: &ProgramState,
        event: EffectEvent,
    ) -> Result<(), OperationalError> {
        if self.events.len() >= state.definition().limits().entries {
            return Err(ProgramError::Capacity.into());
        }
        let mut draft = self.clone();
        if let Some(at) = event.tick() {
            if at < draft.last_tick {
                return Err(OperationalError::ClockRegression);
            }
            draft.last_tick = at;
        }
        draft.apply_inner(state, &event)?;
        draft.events.push(event);
        *self = draft;
        Ok(())
    }
    fn policy(&self, reference: &SourceReference) -> Result<&Policy, OperationalError> {
        self.policies
            .iter()
            .find(|p| &p.reference == reference)
            .ok_or(ProgramError::Missing.into())
    }
    fn grant(&self, id: &GrantId) -> Result<&OperationalGrant, OperationalError> {
        self.grants
            .iter()
            .find(|g| &g.id == id)
            .ok_or(ProgramError::Missing.into())
    }
    fn effect_index(&self, id: &EffectId, scope: &EffectScope) -> Result<usize, OperationalError> {
        let i = self
            .effects
            .iter()
            .position(|e| &e.intent.id == id)
            .ok_or(ProgramError::Missing)?;
        if &self.effects[i].intent.scope != scope {
            return Err(ProgramError::Stale.into());
        }
        Ok(i)
    }
    fn check_grant(
        &self,
        state: &ProgramState,
        grant: &OperationalGrant,
        scope: &EffectScope,
        at: u64,
    ) -> Result<(), OperationalError> {
        if self.revoked.contains(&grant.id) || at < grant.not_before || at >= grant.expires_before {
            return Err(OperationalError::GrantUnavailable);
        }
        if &grant.scope != scope
            || self
                .current_policy()
                .is_none_or(|p| p.reference != grant.policy)
        {
            return Err(ProgramError::Stale.into());
        }
        scope.check_current(state, &grant.principal)?;
        let policy = self.policy(&grant.policy)?;
        if policy.baseline != state.definition().revision()
            || policy.program != *state.definition().id()
        {
            return Err(ProgramError::Stale.into());
        }
        Ok(())
    }
    fn apply_inner(
        &mut self,
        state: &ProgramState,
        event: &EffectEvent,
    ) -> Result<(), OperationalError> {
        match event {
            EffectEvent::Policy(policy) => {
                policy.validate(state)?;
                if policy.mode != self.mode {
                    return Err(OperationalError::ExternalUnavailable);
                }
                if self
                    .policies
                    .iter()
                    .any(|p| p.reference == policy.reference)
                {
                    return Err(ProgramError::IdentityConflict.into());
                }
                // Resource meaning is stable across the retained ledger. A new ID
                // cannot make prior consumption disappear or turn it reusable.
                if self.current_policy().is_some_and(|prior| {
                    resource_set(&prior.capacities) != resource_set(&policy.capacities)
                        || prior.cumulative.iter().cloned().collect::<BTreeSet<_>>()
                            != policy.cumulative.iter().cloned().collect::<BTreeSet<_>>()
                }) {
                    return Err(ProgramError::Structure.into());
                }
                if policy.max_effects < self.effects.len() {
                    return Err(ProgramError::Capacity.into());
                }
                self.policies.push(policy.clone());
            }
            EffectEvent::Grant(grant) => {
                let policy = self.current_policy().ok_or(ProgramError::Missing)?;
                if grant.policy != policy.reference || grant.not_before >= grant.expires_before {
                    return Err(ProgramError::Structure.into());
                }
                if policy.program != grant.scope.program || policy.baseline != grant.scope.baseline
                {
                    return Err(ProgramError::Stale.into());
                }
                if self.grants.iter().any(|g| g.id == grant.id) {
                    return Err(ProgramError::IdentityConflict.into());
                }
                grant.scope.check_current(state, &grant.principal)?;
                validate_quantities(&grant.ceilings, &policy.capacities, false)?;
                self.grants.push(grant.clone());
            }
            EffectEvent::Revoke { grant, .. } => {
                self.grant(grant)?;
                if self.revoked.contains(grant) {
                    return Err(ProgramError::Transition.into());
                }
                self.revoked.push(grant.clone());
                // Revocation neither stops a process nor releases unknown resources.
            }
            EffectEvent::Reserve { intent, at } => {
                if self.effects.iter().any(|e| e.intent.id == intent.id) {
                    return Err(ProgramError::IdentityConflict.into());
                }
                let grant = self.grant(&intent.grant)?;
                self.check_grant(state, grant, &intent.scope, *at)?;
                let policy = self.policy(&grant.policy)?;
                if self.effects.len() >= policy.max_effects {
                    return Err(ProgramError::Capacity.into());
                }
                if self.has_pending_attempt(&intent.scope.attempt) {
                    return Err(ProgramError::Claimed.into());
                }
                validate_quantities(&intent.demand, &policy.capacities, false)?;
                let retries = if let Some(prior_id) = &intent.retry_of {
                    let prior = self
                        .effects
                        .iter()
                        .find(|e| &e.intent.id == prior_id)
                        .ok_or(ProgramError::Missing)?;
                    if prior.intent.scope != intent.scope
                        || !matches!(
                            prior.status,
                            EffectStatus::ReconciledAbsent | EffectStatus::CancelledBeforeDispatch
                        )
                        || self
                            .effects
                            .iter()
                            .any(|e| e.intent.retry_of.as_ref() == Some(prior_id))
                    {
                        return Err(ProgramError::Transition.into());
                    }
                    let count = prior.retries.checked_add(1).ok_or(ProgramError::Capacity)?;
                    if count > policy.max_retries {
                        return Err(ProgramError::Capacity.into());
                    }
                    count
                } else {
                    // Reissuing a logically identical operation with a fresh ID
                    // cannot bypass the safe-retry chain or its finite bound.
                    // This bounded profile permits one chain per ownership attempt.
                    if self
                        .effects
                        .iter()
                        .any(|e| e.intent.scope.attempt == intent.scope.attempt)
                    {
                        return Err(ProgramError::IdentityConflict.into());
                    }
                    0
                };
                for cap in &policy.capacities {
                    if !cap.declared_enforceable {
                        return Err(OperationalError::UnenforceableResource(
                            cap.resource.clone(),
                        ));
                    }
                    let required = quantity(&intent.demand, cap.resource.clone())?;
                    if required > quantity(&grant.ceilings, cap.resource.clone())? {
                        return Err(OperationalError::ResourceLimit(cap.resource.clone()));
                    }
                    let free = cap
                        .available
                        .ok_or(OperationalError::UnknownResource(cap.resource.clone()))?
                        .checked_sub(cap.verification_reserve)
                        .ok_or(OperationalError::ResourceLimit(cap.resource.clone()))?;
                    let total = self
                        .accounted(cap.resource.clone())?
                        .checked_add(required)
                        .ok_or(OperationalError::ResourceLimit(cap.resource.clone()))?;
                    if total > free {
                        return Err(OperationalError::ResourceLimit(cap.resource.clone()));
                    }
                }
                self.effects.push(EffectRecord {
                    intent: intent.clone(),
                    policy: grant.policy.clone(),
                    status: EffectStatus::Reserved,
                    report: None,
                    reconciliation: None,
                    observed_running: false,
                    cancellation_requested: false,
                    retries,
                    process_spec: None,
                    process_observations: Vec::new(),
                    dispatch_at: None,
                });
            }
            EffectEvent::Dispatch {
                effect, scope, at, ..
            } => {
                let i = self.effect_index(effect, scope)?;
                let record = &self.effects[i];
                if record.status != EffectStatus::Reserved {
                    return Err(ProgramError::Transition.into());
                }
                self.check_grant(state, self.grant(&record.intent.grant)?, scope, *at)?;
                if self.mode == EffectMode::SupervisedFixture && record.process_spec.is_none() {
                    return Err(ProgramError::Missing.into());
                }
                self.effects[i].status = EffectStatus::DispatchUncertain;
                self.effects[i].dispatch_at = Some(*at);
            }
            EffectEvent::PrepareProcess {
                effect,
                scope,
                at,
                spec,
            } => {
                if self.mode != EffectMode::SupervisedFixture {
                    return Err(OperationalError::ExternalUnavailable);
                }
                let i = self.effect_index(effect, scope)?;
                let record = &self.effects[i];
                if record.status != EffectStatus::Reserved || record.process_spec.is_some() {
                    return Err(ProgramError::Transition.into());
                }
                let grant = self.grant(&record.intent.grant)?;
                self.check_grant(state, grant, scope, *at)?;
                spec.validate(state.definition().limits())?;
                let policy = self.policy(&record.policy)?;
                if spec.host != policy.host
                    || spec.privacy != policy.privacy
                    || spec.approval != grant.basis
                {
                    return Err(ProgramError::Authority.into());
                }
                if self
                    .effects
                    .iter()
                    .filter_map(|e| e.process_spec.as_ref())
                    .any(|p| {
                        p.reference == spec.reference
                            || p.containment
                                .as_ref()
                                .zip(spec.containment.as_ref())
                                .is_some_and(|(a, b)| {
                                    a.reference == b.reference
                                        || (a.unit == b.unit
                                            && a.delegated_root == b.delegated_root)
                                })
                    })
                {
                    return Err(ProgramError::IdentityConflict.into());
                }
                let mut supported = BTreeSet::from([
                    Resource::WorkerSlots,
                    Resource::ElapsedMillis,
                    Resource::OutputBytes,
                ]);
                if spec.containment.is_some() {
                    supported.extend([
                        Resource::CpuMillicores,
                        spec.containment
                            .as_ref()
                            .and_then(|s| s.memory_resource.clone())
                            .map(Resource::MemoryBytes)
                            .unwrap_or(Resource::UnspecifiedMemoryBytes),
                    ]);
                }
                if resource_set(&policy.capacities) != supported {
                    return Err(ProgramError::EffectUnavailable.into());
                }
                if quantity(&record.intent.demand, Resource::WorkerSlots)? != 1
                    || quantity(&record.intent.demand, Resource::ElapsedMillis)?
                        < match &spec.containment {
                            Some(scope) => scope.elapsed_envelope_ms(&spec.limits),
                            None => spec.limits.wall_ms.checked_add(spec.limits.cleanup_ms),
                        }
                        .ok_or(ProgramError::Capacity)?
                    || quantity(&record.intent.demand, Resource::OutputBytes)?
                        < spec.limits.output_bytes
                {
                    return Err(ProgramError::Capacity.into());
                }
                if let Some(scope) = &spec.containment
                    && (quantity(&record.intent.demand, Resource::CpuMillicores)?
                        < scope
                            .cpu_capacity_millicores()
                            .ok_or(ProgramError::Capacity)?
                        || quantity(&record.intent.demand, scope.memory_dimension())?
                            < scope.controls.memory_bytes)
                {
                    return Err(ProgramError::Capacity.into());
                }
                self.effects[i].process_spec = Some(spec.as_ref().clone());
            }
            EffectEvent::ProcessObserved {
                effect,
                scope,
                observation,
                at,
            } => {
                self.process_observed(state, effect, scope, *at, observation)?;
            }
            EffectEvent::Running { effect, scope, .. } => {
                if self.mode == EffectMode::SupervisedFixture {
                    return Err(ProgramError::EffectUnavailable.into());
                }
                let i = self.effect_index(effect, scope)?;
                let record = &self.effects[i];
                scope.check_current(state, &self.grant(&record.intent.grant)?.principal)?;
                if !matches!(
                    record.status,
                    EffectStatus::DispatchUncertain
                        | EffectStatus::Unknown
                        | EffectStatus::CancellationRequested
                ) {
                    return Err(ProgramError::Transition.into());
                }
                let cancellation_pending = record.cancellation_requested;
                self.effects[i].observed_running = true;
                self.effects[i].status = if cancellation_pending {
                    EffectStatus::CancellationRequested
                } else {
                    EffectStatus::Running
                };
            }
            EffectEvent::Report {
                effect,
                scope,
                outcome,
                ..
            } => {
                if self.mode == EffectMode::SupervisedFixture {
                    return Err(ProgramError::EffectUnavailable.into());
                }
                let i = self.effect_index(effect, scope)?;
                let record = &self.effects[i];
                scope.check_current(state, &self.grant(&record.intent.grant)?.principal)?;
                if record.report.is_some()
                    || !matches!(
                        record.status,
                        EffectStatus::DispatchUncertain
                            | EffectStatus::Running
                            | EffectStatus::Unknown
                            | EffectStatus::CancellationRequested
                    )
                {
                    return Err(ProgramError::Transition.into());
                }
                self.effects[i].report = Some(*outcome);
                self.effects[i].status = EffectStatus::Reported;
            }
            EffectEvent::Unknown { effect, scope, .. } => {
                let i = self.effect_index(effect, scope)?;
                if !matches!(
                    self.effects[i].status,
                    EffectStatus::DispatchUncertain
                        | EffectStatus::Running
                        | EffectStatus::CancellationRequested
                ) {
                    return Err(ProgramError::Transition.into());
                }
                self.effects[i].status = EffectStatus::Unknown;
            }
            EffectEvent::Cancel { effect, scope, .. } => {
                let i = self.effect_index(effect, scope)?;
                self.effects[i].status = match self.effects[i].status {
                    EffectStatus::Reserved => EffectStatus::CancelledBeforeDispatch,
                    EffectStatus::DispatchUncertain
                    | EffectStatus::Running
                    | EffectStatus::Unknown => EffectStatus::CancellationRequested,
                    _ => return Err(ProgramError::Transition.into()),
                };
                self.effects[i].cancellation_requested = true;
            }
            EffectEvent::Reconcile {
                effect,
                scope,
                resolution,
                ..
            } => {
                let i = self.effect_index(effect, scope)?;
                let record = &self.effects[i];
                if !matches!(
                    record.status,
                    EffectStatus::DispatchUncertain
                        | EffectStatus::Running
                        | EffectStatus::Unknown
                        | EffectStatus::CancellationRequested
                        | EffectStatus::Reported
                ) {
                    return Err(ProgramError::Transition.into());
                }
                if self.mode == EffectMode::SupervisedFixture {
                    self.check_process_reconciliation(record, resolution)?;
                }
                let status = match resolution {
                    Reconciliation::Absent => {
                        if record.observed_running || record.report.is_some() {
                            return Err(ProgramError::Transition.into());
                        }
                        EffectStatus::ReconciledAbsent
                    }
                    Reconciliation::Observed { outcome, usage } => {
                        if record.report.is_some_and(|reported| reported != *outcome) {
                            return Err(ProgramError::IdentityConflict.into());
                        }
                        let policy = self.policy(&record.policy)?;
                        validate_quantities(usage, &policy.capacities, true)?;
                        for d in usage {
                            if self.mode == EffectMode::ControlledFixture
                                && quantity(usage, d.resource.clone())?
                                    > quantity(&record.intent.demand, d.resource.clone())?
                            {
                                return Err(OperationalError::ResourceLimit(d.resource.clone()));
                            }
                        }
                        EffectStatus::ReconciledReported
                    }
                };
                self.effects[i].status = status;
                self.effects[i].reconciliation = Some(resolution.clone());
            }
        }
        Ok(())
    }
    fn process_observed(
        &mut self,
        state: &ProgramState,
        effect: &EffectId,
        scope: &EffectScope,
        at: u64,
        observation: &ProcessObservation,
    ) -> Result<(), OperationalError> {
        if self.mode != EffectMode::SupervisedFixture {
            return Err(OperationalError::ExternalUnavailable);
        }
        let i = self.effect_index(effect, scope)?;
        let record = &self.effects[i];
        if !matches!(
            record.status,
            EffectStatus::DispatchUncertain
                | EffectStatus::Running
                | EffectStatus::Unknown
                | EffectStatus::CancellationRequested
        ) {
            return Err(ProgramError::Transition.into());
        }
        let spec = record.process_spec.as_ref().ok_or(ProgramError::Missing)?;
        observation.validate(spec)?;
        let checkpoint = record
            .process_observations
            .iter()
            .any(|o| o.completion == ProcessCompletion::IdentityVerified);
        if spec.identity_checkpoint.is_some() && observation.input_bytes > 0 && !checkpoint {
            return Err(ProgramError::Transition.into());
        }
        if observation.completion == ProcessCompletion::IdentityVerified {
            self.check_supervised_current(state, effect, scope, at)?;
            if checkpoint
                || record.cancellation_requested
                || !record
                    .process_observations
                    .iter()
                    .any(|o| o.completion == ProcessCompletion::Spawned)
                || record.process_observations.iter().any(|o| {
                    matches!(
                        o.completion,
                        ProcessCompletion::StartFailed
                            | ProcessCompletion::Exited { .. }
                            | ProcessCompletion::Interrupted { .. }
                    )
                })
            {
                return Err(ProgramError::Transition.into());
            }
        }
        if self
            .effects
            .iter()
            .flat_map(|e| &e.process_observations)
            .any(|o| o.id == observation.id)
        {
            return Err(ProgramError::IdentityConflict.into());
        }
        for prior in &record.process_observations {
            if let Some(before) = &prior.containment {
                let after = observation
                    .containment
                    .as_ref()
                    .ok_or(ProgramError::Stale)?;
                if before.identity != after.identity
                    || (before.membership_verified && !after.membership_verified)
                    || [
                        (before.cpu_usage_us, after.cpu_usage_us),
                        (before.memory_peak_bytes, after.memory_peak_bytes),
                        (before.tasks_peak, after.tasks_peak),
                    ]
                    .iter()
                    .any(|(a, b)| a.is_some_and(|old| b.is_none_or(|new| new < old)))
                {
                    return Err(ProgramError::Stale.into());
                }
            }
            if prior
                .process
                .as_ref()
                .zip(observation.process.as_ref())
                .is_some_and(|(a, b)| a != b)
            {
                return Err(ProgramError::Stale.into());
            }
            if matches!(
                prior.completion,
                ProcessCompletion::Exited { .. } | ProcessCompletion::Interrupted { .. }
            ) && matches!(
                observation.completion,
                ProcessCompletion::Exited { .. } | ProcessCompletion::Interrupted { .. }
            ) && prior.completion != observation.completion
            {
                return Err(ProgramError::IdentityConflict.into());
            }
        }
        if let Some(prior) = record.process_observations.last()
            && (observation.input_bytes < prior.input_bytes
                || observation.stdout_bytes < prior.stdout_bytes
                || observation.stderr_bytes < prior.stderr_bytes
                || observation.structured_records < prior.structured_records
                || observation.elapsed_ms < prior.elapsed_ms
                || (prior.malformed_output && !observation.malformed_output))
        {
            return Err(ProgramError::Stale.into());
        }
        if observation.completion == ProcessCompletion::Spawned && record.observed_running {
            return Err(ProgramError::Transition.into());
        }
        if observation.completion == ProcessCompletion::StartFailed
            && (record.observed_running || record.report.is_some())
        {
            return Err(ProgramError::Transition.into());
        }
        let outcome = observation.outcome(spec);
        if record.report.zip(outcome).is_some_and(|(a, b)| a != b) {
            return Err(ProgramError::IdentityConflict.into());
        }
        let status = match observation.completion {
            ProcessCompletion::Spawned if record.cancellation_requested => {
                EffectStatus::CancellationRequested
            }
            ProcessCompletion::Spawned | ProcessCompletion::IdentityVerified => {
                EffectStatus::Running
            }
            ProcessCompletion::StartFailed => EffectStatus::Reported,
            ProcessCompletion::Unknown => EffectStatus::Unknown,
            ProcessCompletion::Exited { .. } | ProcessCompletion::Interrupted { .. }
                if observation.quiescent(spec) =>
            {
                EffectStatus::Reported
            }
            _ => EffectStatus::Unknown,
        };
        let record = &mut self.effects[i];
        record.observed_running |= observation.process.is_some()
            || observation.containment.is_some()
            || !observation.unresolved_scope_helpers.is_empty();
        if let Some(outcome) = outcome {
            record.report = Some(outcome);
        }
        record.status = status;
        record.process_observations.push(observation.clone());
        Ok(())
    }
    fn check_process_reconciliation(
        &self,
        record: &EffectRecord,
        resolution: &Reconciliation,
    ) -> Result<(), OperationalError> {
        let spec = record.process_spec.as_ref().ok_or(ProgramError::Missing)?;
        let observation = record
            .process_observations
            .last()
            .ok_or(ProgramError::Missing)?;
        match resolution {
            Reconciliation::Absent => {
                if observation.completion != ProcessCompletion::StartFailed
                    || observation.quiescence != Quiescence::NoChildStarted
                {
                    return Err(ProgramError::Transition.into());
                }
            }
            Reconciliation::Observed { outcome, usage } => {
                if !observation.quiescent(spec) || observation.outcome(spec) != Some(*outcome) {
                    return Err(ProgramError::Transition.into());
                }
                let output = observation
                    .stdout_bytes
                    .checked_add(observation.stderr_bytes)
                    .ok_or(ProgramError::Capacity)?;
                if quantity(usage, Resource::WorkerSlots)? != 1
                    || quantity(usage, Resource::ElapsedMillis)? != observation.elapsed_ms
                    || quantity(usage, Resource::OutputBytes)? != output
                {
                    return Err(ProgramError::Structure.into());
                }
                if spec.identity_checkpoint.is_some()
                    && observation.containment.as_ref().is_none_or(|captured| {
                        captured.cpu_usage_us.is_none()
                            || captured.memory_peak_bytes.is_none()
                            || captured.tasks_peak.is_none()
                    })
                {
                    return Err(ProgramError::Missing.into());
                }
                if let Some(scope) = &spec.containment {
                    let captured = observation
                        .containment
                        .as_ref()
                        .ok_or(ProgramError::Missing)?;
                    if scope.memory_resource.is_some() {
                        // No supported attribution bridge exists from this legacy
                        // scope sample to an explicitly scoped allocation. Native
                        // domain observations have a different, complete identity.
                        return Err(ProgramError::Missing.into());
                    }
                    if quantity(usage, Resource::CpuMillicores)?
                        != scope
                            .cpu_capacity_millicores()
                            .ok_or(ProgramError::Capacity)?
                        || quantity(usage, scope.memory_dimension())?
                            != captured.memory_peak_bytes.ok_or(ProgramError::Missing)?
                    {
                        return Err(ProgramError::Structure.into());
                    }
                }
            }
        }
        Ok(())
    }
}
impl EffectRecord {
    /// Whether resources and ownership must remain held pending reconciliation.
    pub fn pending(&self) -> bool {
        !matches!(
            self.status,
            EffectStatus::CancelledBeforeDispatch
                | EffectStatus::ReconciledAbsent
                | EffectStatus::ReconciledReported
        )
    }
}
impl EffectEvent {
    /// Explicit caller-provided logical tick where the input contains one.
    pub fn tick(&self) -> Option<u64> {
        match self {
            Self::Policy(_) | Self::Grant(_) => None,
            Self::Reserve { at, .. }
            | Self::Revoke { at, .. }
            | Self::Dispatch { at, .. }
            | Self::Running { at, .. }
            | Self::Report { at, .. }
            | Self::Unknown { at, .. }
            | Self::Cancel { at, .. }
            | Self::Reconcile { at, .. }
            | Self::PrepareProcess { at, .. }
            | Self::ProcessObserved { at, .. } => Some(*at),
        }
    }
}
impl EffectScope {
    fn check_current(
        &self,
        state: &ProgramState,
        principal: &ActorId,
    ) -> Result<(), OperationalError> {
        if self.program != *state.definition().id()
            || self.baseline != state.definition().revision()
        {
            return Err(ProgramError::Stale.into());
        }
        let scope = state.readiness().scope(&self.task)?;
        if scope.generation() != self.generation
            || scope.context().contract().candidate() != &self.candidate
        {
            return Err(ProgramError::Stale.into());
        }
        let task = state.readiness().task(&self.task)?;
        // G1 slot observations concern starting another task. This exact
        // ownership attempt is already active; the fixture policy below owns
        // resource admission, including verification reserves and unknown costs.
        if task.status() != TaskStatus::Active
            || !task.dependency_ready()
            || task.blockers().iter().any(|b| {
                !matches!(
                    b,
                    TaskBlocker::NotPending
                        | TaskBlocker::CapacityUnknown
                        | TaskBlocker::NoCapacity
                )
            })
        {
            return Err(ProgramError::Transition.into());
        }
        let attempt = state
            .attempts()
            .iter()
            .find(|a| a.id == self.attempt)
            .ok_or(ProgramError::Missing)?;
        if attempt.owner != *principal
            || attempt.baseline != self.baseline
            || attempt.generation != self.generation
            || attempt.status != AttemptStatus::Claimed
        {
            return Err(ProgramError::Stale.into());
        }
        let increment = state
            .definition()
            .increments()
            .iter()
            .find(|i| i.id == attempt.increment)
            .ok_or(ProgramError::Missing)?;
        if increment.task != self.task {
            return Err(ProgramError::Stale.into());
        }
        Ok(())
    }
}
impl Policy {
    fn validate(&self, state: &ProgramState) -> Result<(), OperationalError> {
        for capacity in &self.capacities {
            capacity.resource.validate_accounting_scope()?;
        }
        if self.mode == EffectMode::ExternalExecutor {
            return Err(OperationalError::ExternalUnavailable);
        }
        if self.program != *state.definition().id()
            || self.baseline != state.definition().revision()
        {
            return Err(ProgramError::Stale.into());
        }
        if self.max_effects == 0
            || self.max_effects > state.definition().limits().entries
            || u64::from(self.max_retries) >= self.max_effects as u64
        {
            return Err(ProgramError::Capacity.into());
        }
        if self.capacities.is_empty()
            || self.capacities.len() > 10
            || resource_set(&self.capacities).len() != self.capacities.len()
            || self.cumulative.len() > self.capacities.len()
            || self
                .cumulative
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>()
                .len()
                != self.cumulative.len()
            || self
                .cumulative
                .iter()
                .any(|r| !self.capacities.iter().any(|c| &c.resource.clone() == r))
        {
            return Err(ProgramError::Structure.into());
        }
        Ok(())
    }
}
fn resource_set(capacities: &[Capacity]) -> BTreeSet<Resource> {
    capacities.iter().map(|c| c.resource.clone()).collect()
}
fn quantity(values: &[Demand], resource: Resource) -> Result<u64, OperationalError> {
    values
        .iter()
        .find(|d| d.resource.clone() == resource)
        .and_then(|d| d.amount)
        .ok_or(OperationalError::UnknownResource(resource.clone()))
}
fn validate_quantities(
    values: &[Demand],
    capacities: &[Capacity],
    allow_zero: bool,
) -> Result<(), OperationalError> {
    if values.len() != capacities.len()
        || values
            .iter()
            .map(|d| d.resource.clone())
            .collect::<BTreeSet<_>>()
            != resource_set(capacities)
    {
        return Err(ProgramError::Structure.into());
    }
    for value in values {
        value.resource.validate_accounting_scope()?;
        let n = value
            .amount
            .ok_or(OperationalError::UnknownResource(value.resource.clone()))?;
        if !allow_zero && n == 0 {
            return Err(OperationalError::ResourceLimit(value.resource.clone()));
        }
    }
    Ok(())
}
