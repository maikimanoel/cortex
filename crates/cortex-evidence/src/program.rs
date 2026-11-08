//! Evidence-backed coordination, with one replayable source-input journal.
//!
//! This is the canonical non-effectful admission boundary. It composes G1/A-1,
//! never upgrades caller assertions to authenticated observations, and never
//! dispatches. Read-only examinations operate on clones. Every admitted release
//! retains the full support contract and observations needed for reconstruction.

use crate::{readiness::*, unified::*};
use cortex_core::{
    TaskId, binding::*, parallel::*, program::context::*, program::operational::*, program::*,
    temporal::SourceReference,
};
use std::{error::Error, fmt, time::SystemTime};

/// Non-disclosing admission refusal retaining useful domain causes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KernelError {
    /// Core contract/transition failed.
    Core(ProgramError),
    /// Unified Evidence refused a transition.
    Evidence(SemanticError),
    /// A-1 rejected malformed/context-incompatible support input.
    Support(SupportInputError),
    /// Executor input preparation refused an exact contract.
    Executor(cortex_core::program::executor::ExecutorError),
    /// Recoverable-domain structural/accounting guard refused the input.
    Domain(cortex_core::program::domain::DomainError),
    /// Controlled-fixture operational guards refused the input.
    Operational(OperationalError),
}
impl fmt::Display for KernelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Domain(e) => e.fmt(f),
            Self::Core(e) => e.fmt(f),
            Self::Evidence(e) => e.fmt(f),
            Self::Support(e) => e.fmt(f),
            Self::Operational(e) => e.fmt(f),
            Self::Executor(e) => e.fmt(f),
        }
    }
}
impl Error for KernelError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(match self {
            Self::Domain(e) => e,
            Self::Core(e) => e,
            Self::Evidence(e) => e,
            Self::Support(e) => e,
            Self::Operational(e) => e,
            Self::Executor(e) => e,
        })
    }
}
impl From<ProgramError> for KernelError {
    fn from(e: ProgramError) -> Self {
        Self::Core(e)
    }
}
impl From<ReadinessError> for KernelError {
    fn from(e: ReadinessError) -> Self {
        Self::Core(e.into())
    }
}
impl From<SemanticError> for KernelError {
    fn from(e: SemanticError) -> Self {
        Self::Evidence(e)
    }
}
impl From<SupportInputError> for KernelError {
    fn from(e: SupportInputError) -> Self {
        Self::Support(e)
    }
}

impl From<cortex_core::program::executor::ExecutorError> for KernelError {
    fn from(e: cortex_core::program::executor::ExecutorError) -> Self {
        Self::Executor(e)
    }
}
impl From<OperationalError> for KernelError {
    fn from(e: OperationalError) -> Self {
        Self::Operational(e)
    }
}

impl From<cortex_core::program::domain::DomainError> for KernelError {
    fn from(e: cortex_core::program::domain::DomainError) -> Self {
        Self::Domain(e)
    }
}

/// Owned exact context for a record examination; absent captures remain absent.
#[derive(Clone, PartialEq, Eq)]
pub struct Examination {
    /// Observation being inspected.
    pub observation: ObservationId,
    /// Exact criterion/candidate contract.
    pub contract: CandidateBindingContract,
    /// Separate caller-supplied current capture.
    pub candidate: Option<CandidateObservation>,
    /// Exact expected profile.
    pub profile: Option<VerificationProfile>,
}
impl Examination {
    fn validation(&self) -> ValidationContext<'_> {
        ValidationContext::new(
            &self.contract,
            self.candidate.as_ref(),
            self.profile.as_ref(),
        )
    }
}
/// Exact original assessment input, replayed through unified Evidence.
#[derive(Clone, PartialEq, Eq)]
pub struct AssessmentInput {
    /// Record/context under examination.
    pub examination: Examination,
    /// Distinct terminal judgment identity.
    pub id: AssessmentId,
    /// Caller-reported assessment time, not an authenticated clock.
    pub at: SystemTime,
    /// Semantic result.
    pub result: AssessmentResult,
    /// Applicability, independent of result/validity.
    pub applicability: Applicability,
    /// Bounded judgment rationale.
    pub rationale: String,
}
/// Exact input to A-1 and the separate coordinator review. The support report
/// cannot confer permission; the independently supplied grant is still required.
#[derive(Clone, PartialEq, Eq)]
pub struct ReleaseInput {
    /// Exact edge/predicate/criterion/artifact contract.
    pub contract: EdgeEvidenceContract,
    /// Current producer capture.
    pub producer: Option<CandidateObservation>,
    /// Current consumer capture.
    pub consumer: Option<CandidateObservation>,
    /// Complete examinations of the referenced records.
    pub examinations: Vec<Examination>,
    /// All required witness assignments, conjunctively evaluated by A-1.
    pub bindings: Vec<WitnessBinding>,
    /// Separate reviewed decision with its exact report, scopes and attestation.
    pub review: Notification,
    /// Positive finite inspection bounds.
    pub limits: SupportLimits,
}
/// Declared non-impact basis for exactly the complement of an amended closure.
/// This is structural coverage over supplied contracts, not semantic discovery.
#[derive(Clone, PartialEq, Eq)]
pub struct NonImpact {
    /// Exact unchanged task identities.
    pub tasks: Vec<TaskId>,
    /// Explicit operator-supplied coverage/rationale reference.
    pub basis: SourceReference,
}
/// Exact non-executing executor input preparation retained for reconstruction.
#[derive(Clone, PartialEq, Eq)]
pub struct ExecutorInputRequest {
    /// Exact stored package identity.
    pub package: PackageId,
    /// Exact stored package revision.
    pub revision: ContractRevision,
    /// Complete declared executor/host capability observations.
    pub descriptor: cortex_core::program::executor::ExecutorDescriptor,
    /// Complete mandatory control requirements.
    pub requirements: cortex_core::program::executor::Requirements,
    /// Explicit input byte bound, no greater than the program envelope.
    pub max_bytes: usize,
}
/// Canonical admitted inputs. Only a separately qualified durable dispatch boundary
/// may perform a supervised fixture effect; kernel admission and replay never do.
#[derive(Clone, PartialEq, Eq)]
pub enum Command {
    /// Version-four global pool/domain input; never performs host operations.
    Domain(Box<cortex_core::program::domain::DomainEvent>),
    /// Register another accepted program in the same pool coordinator journal.
    RegisterDomainMember(Box<ProgramDefinition>),
    /// Apply an existing non-effectful input to a registered member; nesting is refused.
    DomainMember {
        /// Exact registered program.
        program: ProgramId,
        /// Original member operation and its separate scoped authority.
        operation: Box<Operation>,
    },
    /// Informational G1 input; raw Review and Status are prohibited here.
    Notify(Notification),
    /// Original verification observation; no judgment is inferred.
    Observe(ReportedObservation),
    /// Terminal assessment through the existing Evidence guards.
    Assess(Box<AssessmentInput>),
    /// Explicitly retain contextual inspection/invalidation without changing judgment.
    Examine(Examination),
    /// Explicit invalidation retains the original result and first cause.
    InvalidateEvidence {
        /// Existing observation.
        observation: ObservationId,
        /// Bounded supplied reason.
        reason: String,
    },
    /// Support examination and optional per-edge non-effectful release.
    Release(Box<ReleaseInput>),
    /// Ordering only; never an acceptance-baseline mutation.
    OrderingPlan(OrderingPlan),
    /// Exclusive declared responsibility claim.
    Claim(Attempt),
    /// Report, unknown-state reconciliation or cancellation.
    Attempt {
        /// Current attempt identity.
        id: AttemptId,
        /// Exact generation.
        generation: ContractRevision,
        /// Requested metadata status.
        status: AttemptStatus,
        /// Explicit observation/reconciliation basis.
        basis: SourceReference,
    },
    /// Acknowledged responsibility handoff.
    Handoff {
        /// Current attempt.
        old: AttemptId,
        /// Distinct recipient's scoped local acknowledgment.
        recipient: CoordinationGrant,
        /// New attempt identity and responsibility.
        next: Attempt,
    },
    /// New accepted baseline; retain exact prior basis and unchanged coverage.
    Amend {
        /// Validated immutable next baseline.
        next: Box<ProgramDefinition>,
        /// Explicit structural non-impact statement, checked against the closure.
        non_impact: NonImpact,
    },
    /// Assemble and retain an exact non-effectful instruction package.
    Package(PackageRequest),
    /// Controlled fixture input only; never dispatches or creates real authority.
    FixtureEffect(Box<EffectEvent>),
    /// Explicit trusted development-process profile; replay itself never dispatches.
    SupervisedEffect(Box<EffectEvent>),
    /// Retain exact proposed transport input without submission or authority.
    PrepareExecutorInput(Box<ExecutorInputRequest>),
}
/// Complete immutable input, including expected predecessor and local assertion.
#[derive(Clone, PartialEq, Eq)]
pub struct Operation {
    /// Unique store-local operation identity.
    pub id: OperationId,
    /// Expected accepted-operation count (zero at initial creation).
    pub expected: u64,
    /// Exact scoped local metadata authority assertion.
    pub grant: CoordinationGrant,
    /// Full immutable input.
    pub command: Command,
}
/// Retained per-edge support result; original judgments stay in the record pool.
#[derive(Clone, PartialEq, Eq)]
pub struct DependencyDecision {
    /// Admitted operation.
    pub operation: OperationId,
    /// Baseline at the decision.
    pub baseline: ContractRevision,
    /// Exact edge.
    pub edge: EdgeId,
    /// A-1 findings, including all blocked reasons.
    pub findings: Vec<PredicateFinding>,
    /// Whether the distinct G1 review was admitted for a non-effectful consumer.
    pub released: bool,
}
/// Admission receipt reports metadata only, never engineering acceptance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KernelReceipt {
    /// Retained ordinal of this operation.
    pub ordinal: u64,
    /// Exact replay found an existing immutable input; nothing was changed.
    pub duplicate: bool,
}
/// Result of a read-only draft examination; none of these fields grants authority.
#[derive(Clone, PartialEq, Eq)]
pub struct RecordInspection {
    /// Original terminal result, if assessed.
    pub result: Option<AssessmentResult>,
    /// Independent applicability.
    pub applicability: Option<Applicability>,
    /// Contextual validity; absent means correspondence is unresolved.
    pub validity: Option<Validity>,
    /// Typed cause, retaining unknown separately from demonstrated staleness.
    pub issue: Option<ValidityIssue>,
}
/// Rebuildable validated state and immutable input journal.
#[derive(Clone)]
pub struct CoordinationKernel {
    replaying: bool,
    domain_projection: Option<cortex_core::program::domain::DomainProjection>,
    domain_component: bool,
    members: Vec<CoordinationKernel>,
    initial: ProgramDefinition,
    state: ProgramState,
    operations: Vec<Operation>,
    records: Vec<EvidenceRecord>,
    decisions: Vec<DependencyDecision>,
    packages: Vec<InstructionPackage>,
    fixture_effects: OperationalProjection,
    prepared_inputs: Vec<(OperationId, cortex_core::program::executor::PreparedInput)>,
}
impl CoordinationKernel {
    /// Create a kernel from a validated accepted initial baseline.
    pub fn new(initial: ProgramDefinition) -> Result<Self, KernelError> {
        Ok(Self {
            replaying: false,
            domain_projection: None,
            domain_component: false,
            members: vec![],
            state: ProgramState::new(initial.clone())?,
            initial,
            operations: vec![],
            records: vec![],
            decisions: vec![],
            packages: vec![],
            fixture_effects: OperationalProjection::default(),
            prepared_inputs: vec![],
        })
    }
    /// Construct the explicit supervised-development-fixture interpretation.
    /// Its process inputs remain within this same canonical journal; no executor
    /// is invoked by construction, admission, inspection or replay.
    pub fn new_supervised_fixture(initial: ProgramDefinition) -> Result<Self, KernelError> {
        let mut kernel = Self::new(initial)?;
        kernel.fixture_effects = OperationalProjection::new_supervised_fixture();
        Ok(kernel)
    }
    /// Explicit single-operator recoverable-domain fixture coordinator. Every
    /// member and pool allocation is serialized in this same source-input journal.
    /// Legacy execution events are unavailable; no implicit migration occurs.
    pub fn new_domain_fixture(initial: ProgramDefinition) -> Result<Self, KernelError> {
        let mut kernel = Self::new_supervised_fixture(initial)?;
        kernel.domain_projection = Some(Default::default());
        kernel.domain_component = true;
        Ok(kernel)
    }
    /// Reconstruct a complete retained history through a bounded source reader.
    /// Only a fresh kernel is accepted. The historical interpretation is scoped
    /// to this constructor; the returned kernel always uses current admission
    /// guards. This performs no effect and supplies no operational authority.
    pub fn reconstruct<E>(
        mut self,
        mut next: impl FnMut(&Self) -> Result<Option<Operation>, E>,
    ) -> Result<Self, E>
    where
        E: From<KernelError>,
    {
        if !self.operations.is_empty() {
            return Err(KernelError::Core(ProgramError::Structure).into());
        }
        self.replaying = true;
        while let Some(operation) = next(&self)? {
            if self.admit(operation)?.duplicate {
                return Err(KernelError::Core(ProgramError::IdentityConflict).into());
            }
        }
        self.finish_reconstruction();
        Ok(self)
    }
    fn finish_reconstruction(&mut self) {
        self.replaying = false;
        for member in &mut self.members {
            member.finish_reconstruction();
        }
    }
    /// Whether retained input contains explicitly scoped memory representations.
    /// Useful for rejecting projections that cannot represent those semantics.
    pub fn has_scoped_memory(&self) -> bool {
        self.operations.iter().any(|o| {
            command_memory_resources(&o.command).iter().any(|r| {
                matches!(
                    r,
                    cortex_core::program::scheduling::Resource::MemoryBytes(_)
                )
            })
        })
    }
    fn memory_admission_unestablished(&self, command: &Command) -> bool {
        use cortex_core::program::domain::DomainEvent;
        if command_memory_resources(command)
            .iter()
            .any(|r| r.memory_basis_unestablished())
        {
            return true;
        }
        match command {
            Command::FixtureEffect(event) | Command::SupervisedEffect(event) => {
                match event.as_ref() {
                    EffectEvent::Dispatch { effect, .. }
                    | EffectEvent::PrepareProcess { effect, .. } => self
                        .fixture_effects
                        .effects()
                        .iter()
                        .find(|r| &r.intent.id == effect)
                        .is_some_and(|r| {
                            r.intent
                                .demand
                                .iter()
                                .any(|d| d.resource.memory_basis_unestablished())
                        }),
                    _ => false,
                }
            }
            Command::Domain(event) => match event.as_ref() {
                DomainEvent::CreateRequested { domain, .. }
                | DomainEvent::ReleaseRequested { domain, .. } => self
                    .domains()
                    .and_then(|p| p.domain(domain))
                    .is_some_and(|r| {
                        r.intent
                            .effect
                            .demand
                            .iter()
                            .any(|d| d.resource.memory_basis_unestablished())
                    }),
                _ => false,
            },
            Command::DomainMember { program, operation } => self
                .domain_member(program)
                .is_none_or(|member| member.memory_admission_unestablished(&operation.command)),
            _ => false,
        }
    }
    /// Read-only global domain state, when the explicit profile was selected.
    pub fn domains(&self) -> Option<&cortex_core::program::domain::DomainProjection> {
        self.domain_projection.as_ref()
    }
    /// Registered member snapshots. They have no independent store or allocator.
    pub fn domain_members(&self) -> &[CoordinationKernel] {
        &self.members
    }
    /// Find a member, including the coordinator's initial program itself.
    pub fn domain_member(&self, program: &ProgramId) -> Option<&Self> {
        if self.state.definition().id() == program {
            Some(self)
        } else {
            self.members
                .iter()
                .find(|m| m.state.definition().id() == program)
        }
    }
    /// Fresh read-only permission/candidate/package checks. A successful check is
    /// not durable permission, a dispatch receipt or host qualification.
    pub fn check_domain_current(
        &self,
        id: &cortex_core::program::domain::DomainId,
        at: u64,
    ) -> Result<(), KernelError> {
        use cortex_core::program::domain::*;
        let projection = self.domains().ok_or(ProgramError::EffectUnavailable)?;
        let record = projection.domain(id).ok_or(ProgramError::Missing)?;
        if at < projection.last_tick()
            || record.cancelled
            || record.disposition != AdministrativeDisposition::Active
            || record.phase == DomainPhase::Closed
        {
            return Err(ProgramError::Authority.into());
        }
        self.check_domain_intent(&record.intent, at)
    }
    fn check_domain_intent(
        &self,
        intent: &cortex_core::program::domain::DomainIntent,
        at: u64,
    ) -> Result<(), KernelError> {
        let member = self
            .domain_member(&intent.effect.scope.program)
            .ok_or(ProgramError::Missing)?;
        member
            .fixture_effects
            .check_domain_intent(&member.state, &intent.effect, at)?;
        let scope = &intent.effect.scope;
        let package = member
            .packages
            .iter()
            .find(|p| {
                p.request().id == scope.package && p.request().revision == scope.package_revision
            })
            .ok_or(ProgramError::Missing)?;
        if package.request().task != scope.task
            || package.request().baseline != scope.baseline
            || package.request().generation != scope.generation
        {
            return Err(ProgramError::Stale.into());
        }
        package.check(&member.state, package.bytes())?;
        Ok(())
    }
    /// Initial immutable input, used for exact reconstruction.
    pub fn initial(&self) -> &ProgramDefinition {
        &self.initial
    }
    /// Current canonical program projection.
    pub fn state(&self) -> &ProgramState {
        &self.state
    }
    /// All accepted inputs in durable ordinal order.
    pub fn operations(&self) -> &[Operation] {
        &self.operations
    }
    /// Retained original Evidence records and terminal assessments.
    pub fn records(&self) -> &[EvidenceRecord] {
        &self.records
    }
    /// Historical support decisions, not permanent current-release claims.
    pub fn decisions(&self) -> &[DependencyDecision] {
        &self.decisions
    }
    /// Exact assembled bytes and inputs.
    pub fn packages(&self) -> &[InstructionPackage] {
        &self.packages
    }
    /// The selected fixture projection. Its explicit mode separates simulations
    /// from trusted supervised process records; neither is an Evidence assessment.
    pub fn fixture_effects(&self) -> &OperationalProjection {
        &self.fixture_effects
    }
    /// Retained exact transport preparations, identified by their journal operations.
    pub fn prepared_inputs(
        &self,
    ) -> &[(OperationId, cortex_core::program::executor::PreparedInput)] {
        &self.prepared_inputs
    }
    /// Current accepted operation count.
    pub fn revision(&self) -> u64 {
        self.operations.len() as u64
    }
    /// Validate and admit an operation atomically in memory. A durable caller must
    /// reload under its transaction lock and commit the same complete input.
    pub fn admit(&mut self, operation: Operation) -> Result<KernelReceipt, KernelError> {
        if let Command::DomainMember { operation, .. } = &operation.command
            && matches!(
                operation.command,
                Command::Domain(_)
                    | Command::DomainMember { .. }
                    | Command::RegisterDomainMember(_)
            )
        {
            return Err(ProgramError::EffectUnavailable.into());
        }
        if let Some((i, old)) = self
            .operations
            .iter()
            .enumerate()
            .find(|(_, o)| o.id == operation.id)
        {
            return if old == &operation {
                Ok(KernelReceipt {
                    ordinal: i as u64 + 1,
                    duplicate: true,
                })
            } else {
                Err(ProgramError::IdentityConflict.into())
            };
        }
        if !self.replaying && self.memory_admission_unestablished(&operation.command) {
            return Err(OperationalError::UnknownResource(
                cortex_core::program::scheduling::Resource::UnspecifiedMemoryBytes,
            )
            .into());
        }
        if operation.expected != self.revision() {
            return Err(ProgramError::Stale.into());
        }
        if self.operations.len() >= self.state.definition().limits().entries {
            return Err(ProgramError::Capacity.into());
        }
        for old in &self.operations {
            for g in std::iter::once(&old.grant).chain(match &old.command {
                Command::Handoff { recipient, .. } => Some(recipient),
                _ => None,
            }) {
                for incoming in std::iter::once(&operation.grant).chain(match &operation.command {
                    Command::Handoff { recipient, .. } => Some(recipient),
                    _ => None,
                }) {
                    if g.id == incoming.id && g != incoming {
                        return Err(ProgramError::IdentityConflict.into());
                    }
                }
            }
        }
        let mut draft = self.clone();
        draft.reduce(&operation)?;
        draft.operations.push(operation);
        let ordinal = draft.revision();
        *self = draft;
        Ok(KernelReceipt {
            ordinal,
            duplicate: false,
        })
    }
    fn check(
        &self,
        g: &CoordinationGrant,
        p: CoordinationPermission,
        tasks: &[TaskId],
    ) -> Result<(), KernelError> {
        Ok(g.check(self.state.definition(), p, tasks)?)
    }
    fn record_mut(&mut self, id: &ObservationId) -> Result<&mut EvidenceRecord, KernelError> {
        self.records
            .iter_mut()
            .find(|r| r.observation().id() == id)
            .ok_or(ProgramError::Missing.into())
    }
    fn reduce(&mut self, o: &Operation) -> Result<(), KernelError> {
        let g = &o.grant;
        self.check_domain_ownership(self.state.definition().id(), &o.command)?;
        match &o.command {
            Command::RegisterDomainMember(definition) => {
                if self.domain_projection.is_none()
                    || self.members.len() >= self.state.definition().limits().entries
                    || self.domain_member(definition.id()).is_some()
                {
                    return Err(ProgramError::EffectUnavailable.into());
                }
                self.check(
                    g,
                    CoordinationPermission::Amend,
                    &self
                        .state
                        .definition()
                        .graph()
                        .tasks()
                        .iter()
                        .map(|m| m.id().clone())
                        .collect::<Vec<_>>(),
                )?;
                let mut member = Self::new_supervised_fixture((**definition).clone())?;
                member.domain_component = true;
                member.replaying = self.replaying;
                self.members.push(member);
            }
            Command::DomainMember { program, operation } => {
                if self.domain_projection.is_none()
                    || matches!(
                        operation.command,
                        Command::Domain(_)
                            | Command::DomainMember { .. }
                            | Command::RegisterDomainMember(_)
                    )
                {
                    return Err(ProgramError::EffectUnavailable.into());
                }
                self.check(g, CoordinationPermission::Plan, &[])?;
                self.check_domain_ownership(program, &operation.command)?;
                let member = self
                    .members
                    .iter_mut()
                    .find(|m| m.state.definition().id() == program)
                    .ok_or(ProgramError::Missing)?;
                member.admit((**operation).clone())?;
            }
            Command::Domain(event) => {
                use cortex_core::program::domain::*;
                if self.domain_projection.is_none() {
                    return Err(ProgramError::EffectUnavailable.into());
                }
                if matches!(event.as_ref(), DomainEvent::ConfigurePool(_)) {
                    self.check(
                        g,
                        CoordinationPermission::Amend,
                        &self
                            .state
                            .definition()
                            .graph()
                            .tasks()
                            .iter()
                            .map(|m| m.id().clone())
                            .collect::<Vec<_>>(),
                    )?;
                } else {
                    self.check(g, CoordinationPermission::Review, &[])?;
                }
                if let DomainEvent::Reserve { intent, at } = event.as_ref() {
                    self.check_domain_intent(intent, *at)?;
                    let member = self
                        .domain_member(&intent.effect.scope.program)
                        .ok_or(ProgramError::Missing)?;
                    let policy = member
                        .fixture_effects
                        .current_policy()
                        .ok_or(ProgramError::Missing)?;
                    let count = self
                        .domains()
                        .ok_or(ProgramError::Missing)?
                        .domains()
                        .iter()
                        .filter(|d| d.intent.effect.scope.program == intent.effect.scope.program)
                        .count();
                    if count >= policy.max_effects {
                        return Err(ProgramError::Capacity.into());
                    }
                    self.check_domain_member_capacity(intent)?;
                } else if event.needs_current_authority() {
                    self.check_domain_current(
                        event.domain().ok_or(ProgramError::Structure)?,
                        event.tick().ok_or(ProgramError::Structure)?,
                    )?;
                }
                self.domain_projection
                    .as_mut()
                    .ok_or(ProgramError::EffectUnavailable)?
                    .apply((**event).clone())?;
            }
            Command::Notify(n) => {
                if matches!(n.event(), Event::Review(_) | Event::Status { .. }) {
                    return Err(ProgramError::EffectUnavailable.into());
                }
                self.state.notify(g, n.clone())?;
            }
            Command::Observe(r) => {
                self.check(g, CoordinationPermission::Review, &[])?;
                if self
                    .records
                    .iter()
                    .any(|old| old.observation().id() == r.id())
                {
                    return Err(ProgramError::IdentityConflict.into());
                }
                self.records.push(EvidenceRecord::new(r.clone()));
            }
            Command::Assess(a) => {
                self.check(g, CoordinationPermission::Review, &[])?;
                if self
                    .records
                    .iter()
                    .any(|r| r.assessment().is_some_and(|x| x.id() == &a.id))
                {
                    return Err(ProgramError::IdentityConflict.into());
                }
                self.record_mut(&a.examination.observation)?.assess(
                    a.id.clone(),
                    a.at,
                    a.result,
                    a.applicability,
                    &a.rationale,
                    &a.examination.validation(),
                )?;
            }
            Command::Examine(e) => {
                self.check(g, CoordinationPermission::Review, &[])?;
                let current = self
                    .record_mut(&e.observation)?
                    .inspect(&e.validation())
                    .current_pass();
                // Changed relevant Evidence holds the affected previously released consumers.
                if !current {
                    self.invalidate_consumers(g, &o.id, std::slice::from_ref(&e.observation))?;
                }
            }
            Command::InvalidateEvidence {
                observation,
                reason,
            } => {
                self.check(g, CoordinationPermission::Review, &[])?;
                self.record_mut(observation)?.invalidate(reason)?;
                self.invalidate_consumers(g, &o.id, std::slice::from_ref(observation))?;
            }
            Command::Release(r) => {
                let Event::Review(review) = r.review.event() else {
                    return Err(ProgramError::Structure.into());
                };
                if review.edge() != r.contract.scope().edge()
                    || review.scopes()
                        != [r.contract.scope().producer(), r.contract.scope().consumer()]
                {
                    return Err(ProgramError::Stale.into());
                }
                self.check(
                    g,
                    CoordinationPermission::Review,
                    &review
                        .scopes()
                        .iter()
                        .map(|s| s.task().clone())
                        .collect::<Vec<_>>(),
                )?;
                let examinations: Vec<_> = r
                    .examinations
                    .iter()
                    .map(|e| {
                        RecordExamination::new(
                            e.observation.clone(),
                            &e.contract,
                            e.candidate.as_ref(),
                            e.profile.as_ref(),
                        )
                    })
                    .collect();
                let request = SupportRequest::new(
                    r.producer.as_ref(),
                    r.consumer.as_ref(),
                    &examinations,
                    &r.bindings,
                );
                let report = inspect_support(
                    self.state.readiness(),
                    &r.contract,
                    &mut self.records,
                    &request,
                    r.limits,
                )?;
                let supported =
                    report.evidence_part() == EvidencePart::AllRequiredPredicatesSupported;
                let findings = report.findings().to_vec();
                if supported {
                    self.state.notify(g, r.review.clone())?;
                } else {
                    let observations: Vec<_> = r
                        .examinations
                        .iter()
                        .filter_map(|e| {
                            let record = self
                                .records
                                .iter()
                                .find(|record| record.observation().id() == &e.observation)?;
                            let mut draft = record.clone();
                            (draft.inspect(&e.validation()).validity() == Some(Validity::Stale))
                                .then(|| e.observation.clone())
                        })
                        .collect();
                    self.invalidate_consumers(g, &o.id, &observations)?;
                }
                let released = supported
                    && matches!(self.state.readiness().edge(review.edge())?.decision(), EdgeDecision::Released { review: current, .. } if current == r.review.id());
                self.decisions.push(DependencyDecision {
                    operation: o.id.clone(),
                    baseline: g.baseline,
                    edge: review.edge().clone(),
                    findings,
                    released,
                });
            }
            Command::OrderingPlan(p) => self.state.set_ordering_plan(g, p.clone())?,
            Command::Claim(a) => self.state.claim(g, a.clone())?,
            Command::Attempt {
                id,
                generation,
                status,
                basis,
            } => {
                if self.fixture_effects.has_pending_attempt(id) && *status != AttemptStatus::Unknown
                {
                    return Err(ProgramError::Claimed.into());
                }
                self.state
                    .reconcile_attempt(g, id, *generation, *status, basis.clone())?;
            }
            Command::Handoff {
                old,
                recipient,
                next,
            } => {
                if self.fixture_effects.has_pending_attempt(old) {
                    return Err(ProgramError::Claimed.into());
                }
                self.state.handoff(g, recipient, old, next.clone())?;
            }
            Command::Amend { next, non_impact } => {
                let affected = self.state.amend(g, *next.clone())?;
                let mut actual: Vec<_> = self
                    .state
                    .definition()
                    .graph()
                    .tasks()
                    .iter()
                    .filter(|m| !affected.contains(m.id()))
                    .map(|m| m.id().as_str())
                    .collect();
                actual.sort();
                let mut declared: Vec<_> = non_impact.tasks.iter().map(TaskId::as_str).collect();
                declared.sort();
                if actual != declared {
                    return Err(ProgramError::Structure.into());
                }
            }
            Command::FixtureEffect(event) => {
                if self.fixture_effects.mode() != EffectMode::ControlledFixture {
                    return Err(ProgramError::EffectUnavailable.into());
                }
                self.fixture_event(g, event)?;
            }
            Command::SupervisedEffect(event) => {
                if self.fixture_effects.mode() != EffectMode::SupervisedFixture {
                    return Err(ProgramError::EffectUnavailable.into());
                }
                self.fixture_event(g, event)?;
            }
            Command::PrepareExecutorInput(request) => {
                for (_, previous) in &self.prepared_inputs {
                    if (previous.descriptor().reference() == request.descriptor.reference()
                        && previous.descriptor() != &request.descriptor)
                        || (previous.requirements().reference() == request.requirements.reference()
                            && previous.requirements() != &request.requirements)
                    {
                        return Err(ProgramError::IdentityConflict.into());
                    }
                }
                let package = self
                    .packages
                    .iter()
                    .find(|p| {
                        p.request().id == request.package
                            && p.request().revision == request.revision
                    })
                    .ok_or(ProgramError::Missing)?;
                self.check(
                    g,
                    CoordinationPermission::Plan,
                    std::slice::from_ref(&package.request().task),
                )?;
                let prepared = cortex_core::program::executor::prepare(
                    &self.state,
                    package,
                    &request.descriptor,
                    &request.requirements,
                    request.max_bytes,
                )?;
                self.prepared_inputs.push((o.id.clone(), prepared));
            }
            Command::Package(p) => {
                self.check(
                    g,
                    CoordinationPermission::Plan,
                    std::slice::from_ref(&p.task),
                )?;
                if self.packages.iter().any(|old| old.request().id == p.id) {
                    return Err(ProgramError::IdentityConflict.into());
                }
                self.packages.push(assemble(&self.state, p.clone())?);
            }
        }
        Ok(())
    }
    fn check_domain_member_capacity(
        &self,
        intent: &cortex_core::program::domain::DomainIntent,
    ) -> Result<(), KernelError> {
        use cortex_core::program::{domain::*, scheduling::Resource};
        let member = self
            .domain_member(&intent.effect.scope.program)
            .ok_or(ProgramError::Missing)?;
        let policy = member
            .fixture_effects
            .current_policy()
            .ok_or(ProgramError::Missing)?;
        let projection = self.domains().ok_or(ProgramError::Missing)?;
        for capacity in &policy.capacities {
            let resource = capacity.resource.clone();
            let requested = intent
                .effect
                .demand
                .iter()
                .find(|d| d.resource.clone() == resource)
                .and_then(|d| d.amount)
                .ok_or(OperationalError::UnknownResource(resource.clone()))?;
            let cumulative = policy.cumulative.contains(&resource);
            let dimension = match (resource.clone(), cumulative) {
                (Resource::ElapsedMillis, true) => Some(Consumption::ElapsedMillis),
                (Resource::OutputBytes, true) => Some(Consumption::OutputBytes),
                (
                    Resource::WorkerSlots
                    | Resource::BuildJobs
                    | Resource::TestProcesses
                    | Resource::CpuMillicores
                    | Resource::UnspecifiedMemoryBytes
                    | Resource::MemoryBytes(_)
                    | Resource::DiskBytes,
                    false,
                ) => None,
                // Rate/capacity cannot silently become cumulative execution time.
                // Provider/CI and other unsupported mappings are not this profile.
                _ => return Err(ProgramError::Structure.into()),
            };
            if let Some(dimension) = dimension
                && (!projection
                    .pool()
                    .is_some_and(|p| p.consumption.iter().any(|b| b.dimension == dimension))
                    || intent
                        .consumption
                        .iter()
                        .find(|d| d.dimension == dimension)
                        .is_none_or(|d| d.amount != requested))
            {
                return Err(ProgramError::Structure.into());
            }
            let mut used = 0u64;
            for prior in projection
                .domains()
                .iter()
                .filter(|d| d.intent.effect.scope.program == intent.effect.scope.program)
            {
                let amount = if let Some(dimension) = dimension {
                    match prior.phase {
                        DomainPhase::CancelledBeforeCreate => 0,
                        DomainPhase::Reserved => prior
                            .intent
                            .consumption
                            .iter()
                            .find(|d| d.dimension == dimension)
                            .map(|d| d.amount)
                            .ok_or(ProgramError::Structure)?,
                        _ => prior
                            .measured_consumption(dimension)
                            .ok_or(DomainError::Debt(dimension))?,
                    }
                } else if prior.holds(resource.clone()) {
                    prior
                        .intent
                        .effect
                        .demand
                        .iter()
                        .find(|d| d.resource.clone() == resource)
                        .and_then(|d| d.amount)
                        .ok_or(OperationalError::UnknownResource(resource.clone()))?
                } else {
                    0
                };
                used = used
                    .checked_add(amount)
                    .ok_or(OperationalError::ResourceLimit(resource.clone()))?;
            }
            let free = capacity
                .available
                .ok_or(OperationalError::UnknownResource(resource.clone()))?
                .checked_sub(capacity.verification_reserve)
                .ok_or(OperationalError::ResourceLimit(resource.clone()))?;
            if used.checked_add(requested).is_none_or(|n| n > free) {
                return Err(OperationalError::ResourceLimit(resource.clone()).into());
            }
        }
        Ok(())
    }
    fn check_domain_ownership(
        &self,
        program: &ProgramId,
        command: &Command,
    ) -> Result<(), KernelError> {
        use cortex_core::program::domain::{DomainPhase, HistoricalOutcome};
        let attempt = match command {
            Command::Attempt { id, .. } => Some(id),
            Command::Handoff { old, .. } => Some(old),
            _ => None,
        };
        if let (Some(domains), Some(attempt)) = (&self.domain_projection, attempt)
            && domains.domains().iter().any(|d| {
                &d.intent.effect.scope.program == program
                    && &d.intent.effect.scope.attempt == attempt
                    && (!matches!(
                        d.phase,
                        DomainPhase::Closed | DomainPhase::CancelledBeforeCreate
                    ) || d.outcome == HistoricalOutcome::Unknown)
            })
        {
            return Err(ProgramError::Claimed.into());
        }
        Ok(())
    }
    fn fixture_event(
        &mut self,
        grant: &CoordinationGrant,
        event: &EffectEvent,
    ) -> Result<(), KernelError> {
        if self.domain_component
            && !matches!(
                event,
                EffectEvent::Policy(_) | EffectEvent::Grant(_) | EffectEvent::Revoke { .. }
            )
        {
            return Err(ProgramError::EffectUnavailable.into());
        }
        let tasks = match event {
            EffectEvent::Policy(_) => self
                .state
                .definition()
                .graph()
                .tasks()
                .iter()
                .map(|m| m.id().clone())
                .collect(),
            EffectEvent::Grant(g) => vec![g.scope.task.clone()],
            EffectEvent::Reserve { intent, .. } => vec![intent.scope.task.clone()],
            EffectEvent::Revoke { grant, .. } => vec![
                self.fixture_effects
                    .grants()
                    .iter()
                    .find(|g| &g.id == grant)
                    .ok_or(ProgramError::Missing)?
                    .scope
                    .task
                    .clone(),
            ],
            EffectEvent::Dispatch { scope, .. }
            | EffectEvent::PrepareProcess { scope, .. }
            | EffectEvent::ProcessObserved { scope, .. }
            | EffectEvent::Running { scope, .. }
            | EffectEvent::Report { scope, .. }
            | EffectEvent::Unknown { scope, .. }
            | EffectEvent::Cancel { scope, .. }
            | EffectEvent::Reconcile { scope, .. } => vec![scope.task.clone()],
        };
        self.check(grant, CoordinationPermission::Review, &tasks)?;
        let scope = match event {
            EffectEvent::Reserve { intent, .. } => Some(&intent.scope),
            EffectEvent::Dispatch { scope, .. } | EffectEvent::PrepareProcess { scope, .. } => {
                Some(scope)
            }
            _ => None,
        };
        if let Some(scope) = scope {
            let package = self
                .packages
                .iter()
                .find(|p| {
                    p.request().id == scope.package
                        && p.request().revision == scope.package_revision
                })
                .ok_or(ProgramError::Missing)?;
            let request = package.request();
            if request.task != scope.task
                || request.baseline != scope.baseline
                || request.generation != scope.generation
            {
                return Err(ProgramError::Stale.into());
            }
            package.check(&self.state, package.bytes())?;
        }
        let process = match event {
            EffectEvent::PrepareProcess { scope, spec, .. } => Some((scope, spec.as_ref())),
            EffectEvent::Dispatch { effect, scope, .. }
                if self.fixture_effects.mode() == EffectMode::SupervisedFixture =>
            {
                Some((
                    scope,
                    self.fixture_effects
                        .process_spec(effect)
                        .ok_or(ProgramError::Missing)?,
                ))
            }
            _ => None,
        };
        if let Some((scope, spec)) = process {
            let prepared = self
                .prepared_inputs
                .iter()
                .find(|(id, _)| id == &spec.prepared_input)
                .map(|(_, input)| input)
                .ok_or(ProgramError::Missing)?;
            let request = prepared.package().request();
            if request.id != scope.package
                || request.revision != scope.package_revision
                || request.task != scope.task
                || request.baseline != scope.baseline
                || request.generation != scope.generation
                || prepared.bytes().len() as u64 != spec.input_len
                || prepared.descriptor().identity().executable() != &spec.executable_identity
                || prepared.descriptor().identity().host() != &spec.host
                || prepared.requirements().reference() != &spec.profile
                || spec.cwd != scope.candidate.repository().root()
            {
                return Err(ProgramError::Stale.into());
            }
            prepared.check(
                &self.state,
                prepared.descriptor(),
                prepared.requirements(),
                prepared.bytes(),
            )?;
        }
        self.fixture_effects.apply(&self.state, event.clone())?;
        Ok(())
    }
    // Recompute original support requests after explicit validity mutations. Only
    // genuinely unsupported released consumers seed G1's existing causal closure.
    fn invalidate_consumers(
        &mut self,
        g: &CoordinationGrant,
        id: &OperationId,
        observations: &[ObservationId],
    ) -> Result<(), KernelError> {
        let mut roots = Vec::new();
        for op in &self.operations {
            let Command::Release(r) = &op.command else {
                continue;
            };
            if !r
                .bindings
                .iter()
                .any(|b| observations.contains(b.evidence().observation()))
            {
                continue;
            }
            let Ok(edge) = self.state.readiness().edge(r.contract.scope().edge()) else {
                continue;
            };
            if !matches!(edge.decision(),EdgeDecision::Released{review,..} if review==r.review.id())
            {
                continue;
            }
            let scope = self
                .state
                .readiness()
                .scope(r.contract.scope().consumer().task())?;
            if !roots.contains(&scope) {
                roots.push(scope);
            }
        }
        if !roots.is_empty() {
            let cause = SourceReference::operator_reported(
                g.actor.as_str(),
                "Evidence context changed",
                id.as_str(),
            )
            .map_err(|_| ProgramError::Structure)?;
            let n = Notification::new(
                NotificationId::new(id.as_str())?,
                self.state.definition().graph().reference().clone(),
                Event::Invalidate {
                    roots,
                    cause,
                    coverage: DependencyCoverage::DeclaredComplete,
                },
            )?;
            self.state.notify(g, n)?;
        }
        Ok(())
    }
    /// Examine a draft without mutating stored Evidence or canonical events.
    pub fn inspect_record(
        &self,
        examination: &Examination,
    ) -> Result<RecordInspection, KernelError> {
        let mut r = self
            .records
            .iter()
            .find(|r| r.observation().id() == &examination.observation)
            .ok_or(ProgramError::Missing)?
            .clone();
        let v = r.inspect(&examination.validation());
        Ok(RecordInspection {
            result: v.result(),
            applicability: v.applicability(),
            validity: v.validity(),
            issue: v.validity_issue().cloned(),
        })
    }
}

// One traversal owns the new-admission boundary and machine projection detection.
// Historical reconstruction retains old quantities without assigning them a basis.
fn command_memory_resources(command: &Command) -> Vec<cortex_core::program::scheduling::Resource> {
    use cortex_core::program::{domain::DomainEvent, scheduling::Resource};
    fn effect(event: &EffectEvent) -> Vec<Resource> {
        match event {
            EffectEvent::Policy(p) => p
                .capacities
                .iter()
                .map(|c| c.resource.clone())
                .chain(p.cumulative.iter().cloned())
                .collect(),
            EffectEvent::Grant(g) => g.ceilings.iter().map(|d| d.resource.clone()).collect(),
            EffectEvent::Reserve { intent, .. } => {
                intent.demand.iter().map(|d| d.resource.clone()).collect()
            }
            EffectEvent::Reconcile {
                resolution:
                    cortex_core::program::operational::Reconciliation::Observed { usage, .. },
                ..
            } => usage.iter().map(|d| d.resource.clone()).collect(),
            EffectEvent::PrepareProcess { spec, .. } => spec
                .containment
                .as_ref()
                .and_then(|s| s.memory_resource.clone())
                .map(|m| vec![Resource::MemoryBytes(m)])
                .unwrap_or_default(),
            _ => vec![],
        }
    }
    match command {
        Command::PrepareExecutorInput(request) => {
            use cortex_core::program::executor::Control;
            request
                .descriptor
                .observations()
                .iter()
                .map(|o| &o.control)
                .chain(request.requirements.required().iter().map(|r| &r.control))
                .filter_map(|c| match c {
                    Control::Resource(r) => Some(r.clone()),
                    _ => None,
                })
                .collect()
        }
        Command::FixtureEffect(e) | Command::SupervisedEffect(e) => effect(e),
        Command::DomainMember { operation, .. } => command_memory_resources(&operation.command),
        Command::Domain(e) => match e.as_ref() {
            DomainEvent::ConfigurePool(p) => {
                p.capacities.iter().map(|c| c.resource.clone()).collect()
            }
            DomainEvent::Reserve { intent, .. } => intent
                .effect
                .demand
                .iter()
                .map(|d| d.resource.clone())
                .collect(),
            DomainEvent::Closed { receipt, .. }
            | DomainEvent::RecoveredClosed { receipt, .. }
            | DomainEvent::Settle { receipt, .. } => receipt.released.clone(),
            _ => vec![],
        },
        _ => vec![],
    }
}
