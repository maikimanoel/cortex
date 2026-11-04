//! Evidence-backed Stage 1 coordination, with a replayable source-input journal.
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
    /// Controlled-fixture operational guards refused the input.
    Operational(OperationalError),
}
impl fmt::Display for KernelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
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
    /// Exact unchanged mission identities.
    pub missions: Vec<TaskId>,
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
/// Canonical admitted command families. No real external-effect command exists.
#[derive(Clone, PartialEq, Eq)]
pub enum Command {
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
    Plan(Plan),
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
    /// Reconstructed controlled fixtures, separate from real effects and Evidence.
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
        missions: &[TaskId],
    ) -> Result<(), KernelError> {
        Ok(g.check(self.state.definition(), p, missions)?)
    }
    fn record_mut(&mut self, id: &ObservationId) -> Result<&mut EvidenceRecord, KernelError> {
        self.records
            .iter_mut()
            .find(|r| r.observation().id() == id)
            .ok_or(ProgramError::Missing.into())
    }
    fn reduce(&mut self, o: &Operation) -> Result<(), KernelError> {
        let g = &o.grant;
        match &o.command {
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
                        .map(|s| s.mission().clone())
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
            Command::Plan(p) => self.state.set_plan(g, p.clone())?,
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
                    .missions()
                    .iter()
                    .filter(|m| !affected.contains(m.id()))
                    .map(|m| m.id().as_str())
                    .collect();
                actual.sort();
                let mut declared: Vec<_> = non_impact.missions.iter().map(TaskId::as_str).collect();
                declared.sort();
                if actual != declared {
                    return Err(ProgramError::Structure.into());
                }
            }
            Command::FixtureEffect(event) => self.fixture_event(g, event)?,
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
                    std::slice::from_ref(&package.request().mission),
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
                    std::slice::from_ref(&p.mission),
                )?;
                if self.packages.iter().any(|old| old.request().id == p.id) {
                    return Err(ProgramError::IdentityConflict.into());
                }
                self.packages.push(assemble(&self.state, p.clone())?);
            }
        }
        Ok(())
    }
    fn fixture_event(
        &mut self,
        grant: &CoordinationGrant,
        event: &EffectEvent,
    ) -> Result<(), KernelError> {
        let missions = match event {
            EffectEvent::Policy(_) => self
                .state
                .definition()
                .graph()
                .missions()
                .iter()
                .map(|m| m.id().clone())
                .collect(),
            EffectEvent::Grant(g) => vec![g.scope.mission.clone()],
            EffectEvent::Reserve { intent, .. } => vec![intent.scope.mission.clone()],
            EffectEvent::Revoke { grant, .. } => vec![
                self.fixture_effects
                    .grants()
                    .iter()
                    .find(|g| &g.id == grant)
                    .ok_or(ProgramError::Missing)?
                    .scope
                    .mission
                    .clone(),
            ],
            EffectEvent::Dispatch { scope, .. }
            | EffectEvent::Running { scope, .. }
            | EffectEvent::Report { scope, .. }
            | EffectEvent::Unknown { scope, .. }
            | EffectEvent::Cancel { scope, .. }
            | EffectEvent::Reconcile { scope, .. } => vec![scope.mission.clone()],
        };
        self.check(grant, CoordinationPermission::Review, &missions)?;
        let scope = match event {
            EffectEvent::Reserve { intent, .. } => Some(&intent.scope),
            EffectEvent::Dispatch { scope, .. } => Some(scope),
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
            if request.mission != scope.mission
                || request.baseline != scope.baseline
                || request.generation != scope.generation
            {
                return Err(ProgramError::Stale.into());
            }
            package.check(&self.state, package.bytes())?;
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
                .scope(r.contract.scope().consumer().mission())?;
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
