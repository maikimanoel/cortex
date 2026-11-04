//! GPE-B Stage 1: bounded, non-effectful program contracts and coordination.
//!
//! Explicit operator references are assertions, not authenticated grants. A
//! claim reserves declared metadata responsibility, never a process or host
//! resource. Execution authority is always unavailable. Evidence owns release
//! support; storage must replay accepted inputs rather than trust projections.

use crate::{
    ContractError, TaskId, binding::ContractRevision, parallel::*, temporal::SourceReference,
};
use std::{collections::BTreeSet, error::Error, fmt};

pub mod context;
pub mod scheduling;

macro_rules! id {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);
        impl $name {
            /// Validate a bounded foundation identifier, without authentication.
            pub fn new(value: &str) -> Result<Self, ProgramError> {
                crate::validate_identifier(stringify!($name), value)?;
                Ok(Self(value.to_owned()))
            }
            /// Inspect explicitly supplied identity text.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(concat!(stringify!($name), "([redacted])"))
            }
        }
    };
}
id!(
    ProgramId,
    "Program identity, separate from a task or candidate."
);
id!(
    WorkstreamId,
    "Long-lived responsibility domain within a program."
);
id!(
    IncrementId,
    "Intermediate deliverable identity within a program."
);
id!(
    AttemptId,
    "A unique metadata attempt, never an executor session."
);
id!(
    ActorId,
    "Caller-declared local operator or responsibility owner."
);
id!(
    OperationId,
    "Immutable admission identity within a store incarnation."
);
id!(GrantId, "An explicit local metadata grant reference.");
id!(
    ResourceId,
    "Declared exclusive semantic responsibility, not a host limit."
);
id!(PackageId, "An immutable instruction package identity.");

/// Non-disclosing refusal with typed domain causes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProgramError {
    /// Foundation validation failed.
    Contract(ContractError),
    /// Existing graph semantics refused the input.
    Readiness(ReadinessError),
    /// Invalid or inconsistent program membership/contracts.
    Structure,
    /// A finite caller-selected count or byte limit was reached.
    Capacity,
    /// Expected state/baseline/generation differs.
    Stale,
    /// An identity already has different contents.
    IdentityConflict,
    /// A current responsibility conflicts with another claim.
    Claimed,
    /// Exact scoped local metadata grant was not supplied.
    Authority,
    /// No such current claim/attempt/reference exists.
    Missing,
    /// An attempted transition is unsupported or terminal.
    Transition,
    /// Stage 1 cannot perform or authorize this effect.
    EffectUnavailable,
}
impl fmt::Display for ProgramError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Contract(_) => f.write_str("program: invalid contract"),
            Self::Readiness(_) => f.write_str("program: invalid readiness input"),
            other => write!(f, "program: {other:?}"),
        }
    }
}
impl Error for ProgramError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Contract(e) => Some(e),
            Self::Readiness(e) => Some(e),
            _ => None,
        }
    }
}
impl From<ContractError> for ProgramError {
    fn from(e: ContractError) -> Self {
        Self::Contract(e)
    }
}
impl From<ReadinessError> for ProgramError {
    fn from(e: ReadinessError) -> Self {
        Self::Readiness(e)
    }
}

/// Positive finite logical bounds. These are not allocator or host quotas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProgramLimits {
    /// Retained baseline revisions.
    pub revisions: usize,
    /// Accepted inputs, attempts and packages, each independently bounded.
    pub entries: usize,
    /// Maximum complete encoded journal bytes, also enforced by the codec.
    pub bytes: usize,
}
impl ProgramLimits {
    /// Validate positive, representable input bounds.
    pub fn new(revisions: usize, entries: usize, bytes: usize) -> Result<Self, ProgramError> {
        if revisions == 0 || entries == 0 || bytes == 0 || bytes > 64 * 1024 * 1024 {
            return Err(ProgramError::Capacity);
        }
        Ok(Self {
            revisions,
            entries,
            bytes,
        })
    }
}

/// Untrusted membership input, validated by `ProgramDefinition::new`.
#[derive(Clone, PartialEq, Eq)]
pub struct Workstream {
    /// Domain identity.
    pub id: WorkstreamId,
    /// Protected responsibility description.
    pub responsibility: String,
    /// Exact graph missions owned by this domain.
    pub missions: Vec<TaskId>,
}
/// Untrusted increment input, validated against the graph and other increments.
#[derive(Clone, PartialEq, Eq)]
pub struct Increment {
    /// Unique increment identity.
    pub id: IncrementId,
    /// Producer mission.
    pub mission: TaskId,
    /// Exact intermediate artifacts; empty is allowed for a terminal-only increment.
    pub artifacts: Vec<ArtifactReference>,
    /// Explicit exclusive semantic scopes, including conflicts beyond path overlap.
    pub resources: Vec<ResourceId>,
}
/// Protected program-wide outcome, distinct from its individual mission criteria.
#[derive(Clone, PartialEq, Eq)]
pub struct ProgramOutcome {
    objective: String,
    acceptance: String,
}
impl ProgramOutcome {
    /// Validate finite objective and final acceptance text before retention.
    pub fn new(objective: &str, acceptance: &str) -> Result<Self, ProgramError> {
        crate::validate_text("program_objective", objective)?;
        crate::validate_text("program_acceptance", acceptance)?;
        Ok(Self {
            objective: objective.to_owned(),
            acceptance: acceptance.to_owned(),
        })
    }
    /// Accepted program objective.
    pub fn objective(&self) -> &str {
        &self.objective
    }
    /// Protected program-level outcome criterion, never implied by worker completion.
    pub fn acceptance(&self) -> &str {
        &self.acceptance
    }
}
/// Protected, versioned acceptance baseline; its graph and clauses are immutable.
#[derive(Clone, PartialEq, Eq)]
pub struct ProgramDefinition {
    id: ProgramId,
    outcome: ProgramOutcome,
    revision: ContractRevision,
    predecessor: Option<ContractRevision>,
    approval: SourceReference,
    graph: DependencyGraph,
    workstreams: Vec<Workstream>,
    increments: Vec<Increment>,
    limits: ProgramLimits,
}
impl ProgramDefinition {
    /// Validate exact ownership, unique increments/artifacts and finite bounds.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: ProgramId,
        outcome: ProgramOutcome,
        revision: ContractRevision,
        predecessor: Option<ContractRevision>,
        approval: SourceReference,
        graph: DependencyGraph,
        workstreams: Vec<Workstream>,
        increments: Vec<Increment>,
        limits: ProgramLimits,
    ) -> Result<Self, ProgramError> {
        ProgramLimits::new(limits.revisions, limits.entries, limits.bytes)?;
        if predecessor.is_some_and(|p| p.get().checked_add(1) != Some(revision.get()))
            || (predecessor.is_none() && revision.get() != 1)
        {
            return Err(ProgramError::Structure);
        }
        if workstreams.is_empty()
            || increments.is_empty()
            || workstreams.len() > limits.entries
            || increments.len() > limits.entries
        {
            return Err(ProgramError::Capacity);
        }
        let mut streams = BTreeSet::new();
        let mut owned = BTreeSet::new();
        let mut text_bytes = 0usize;
        for w in &workstreams {
            crate::validate_text("responsibility", &w.responsibility)?;
            text_bytes = text_bytes
                .checked_add(w.responsibility.len())
                .ok_or(ProgramError::Capacity)?;
            if !streams.insert(&w.id) || w.missions.is_empty() {
                return Err(ProgramError::Structure);
            }
            for m in &w.missions {
                if !owned.insert(m.as_str()) {
                    return Err(ProgramError::Structure);
                }
            }
        }
        if text_bytes > limits.bytes
            || owned != graph.missions().iter().map(|m| m.id().as_str()).collect()
        {
            return Err(ProgramError::Structure);
        }
        let mut ids = BTreeSet::new();
        let mut artifacts = BTreeSet::new();
        for i in &increments {
            if !ids.insert(&i.id)
                || !owned.contains(i.mission.as_str())
                || i.resources.len() > limits.entries
                || i.artifacts.len() > limits.entries
                || i.resources.iter().collect::<BTreeSet<_>>().len() != i.resources.len()
            {
                return Err(ProgramError::Structure);
            }
            for a in &i.artifacts {
                if !artifacts.insert((a.id().as_str(), a.revision().get()))
                    || !graph
                        .edges()
                        .iter()
                        .any(|e| e.producer() == &i.mission && e.requirement().artifact() == a)
                {
                    return Err(ProgramError::Structure);
                }
            }
        }
        if graph
            .missions()
            .iter()
            .any(|m| !increments.iter().any(|i| &i.mission == m.id()))
            || graph.edges().iter().any(|e| {
                !increments.iter().any(|i| {
                    &i.mission == e.producer() && i.artifacts.contains(e.requirement().artifact())
                })
            })
        {
            return Err(ProgramError::Structure);
        }
        Ok(Self {
            id,
            outcome,
            revision,
            predecessor,
            approval,
            graph,
            workstreams,
            increments,
            limits,
        })
    }
    /// Program identity.
    pub fn id(&self) -> &ProgramId {
        &self.id
    }
    /// Protected overall accepted outcome.
    pub fn outcome(&self) -> &ProgramOutcome {
        &self.outcome
    }
    /// Immutable accepted revision.
    pub fn revision(&self) -> ContractRevision {
        self.revision
    }
    /// Prior acceptance basis, if amended.
    pub fn predecessor(&self) -> Option<ContractRevision> {
        self.predecessor
    }
    /// Explicit caller-reported owner acceptance basis.
    pub fn approval(&self) -> &SourceReference {
        &self.approval
    }
    /// Canonical graph contract.
    pub fn graph(&self) -> &DependencyGraph {
        &self.graph
    }
    /// Responsibility domains.
    pub fn workstreams(&self) -> &[Workstream] {
        &self.workstreams
    }
    /// Intermediate delivery contracts.
    pub fn increments(&self) -> &[Increment] {
        &self.increments
    }
    /// Finite retained-input bounds.
    pub fn limits(&self) -> ProgramLimits {
        self.limits
    }
}

/// Explicit trusted-local metadata permission. No effectful permission exists here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoordinationPermission {
    /// Amend the protected acceptance baseline with an owner reference.
    Amend,
    /// Edit priorities, propose packages and update informational readiness.
    Plan,
    /// Claim, hand over and reconcile declared work responsibility.
    Claim,
    /// Admit inspected dependency coordination decisions and invalidation.
    Review,
}
/// Exact baseline-scoped operator assertion; stored with each admitted operation.
#[derive(Clone, PartialEq, Eq)]
pub struct CoordinationGrant {
    /// Immutable grant identity; conflicting reuse is refused by the journal owner.
    pub id: GrantId,
    /// Applicable program.
    pub program: ProgramId,
    /// Applicable protected baseline.
    pub baseline: ContractRevision,
    /// Claimed invoking operator.
    pub actor: ActorId,
    /// Explicit covered missions.
    pub missions: Vec<TaskId>,
    /// Explicit metadata operations.
    pub permissions: Vec<CoordinationPermission>,
    /// Caller-reported authority basis, not a signature.
    pub basis: SourceReference,
}
impl CoordinationGrant {
    /// Validate correspondence and required permission/mission scope.
    pub fn check(
        &self,
        definition: &ProgramDefinition,
        permission: CoordinationPermission,
        missions: &[TaskId],
    ) -> Result<(), ProgramError> {
        if self.program != definition.id
            || self.baseline != definition.revision
            || !self.permissions.contains(&permission)
            || self.permissions.len() > 4
            || self
                .permissions
                .iter()
                .enumerate()
                .any(|(i, p)| self.permissions[..i].contains(p))
            || self.missions.len() > definition.graph.missions().len()
            || self
                .missions
                .iter()
                .map(TaskId::as_str)
                .collect::<BTreeSet<_>>()
                .len()
                != self.missions.len()
            || self
                .missions
                .iter()
                .any(|m| !definition.graph.missions().iter().any(|x| x.id() == m))
            || missions.iter().any(|m| !self.missions.contains(m))
        {
            return Err(ProgramError::Authority);
        }
        Ok(())
    }
}

/// Metadata attempt lifecycle. No state means a real process was run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttemptStatus {
    /// Current ownership is retained.
    Claimed,
    /// Operator reports an unresolved simulated outcome; blocks replacement.
    Unknown,
    /// Work report retained, awaiting distinct review.
    Reported,
    /// Explicitly cancelled, never rolled back or accepted.
    Cancelled,
    /// Ownership handed to a named new actor by both acknowledgments.
    HandedOver,
}
/// Retained exact attempt/generation and responsibility, never worker authority.
#[derive(Clone, PartialEq, Eq)]
pub struct Attempt {
    /// Immutable attempt identity.
    pub id: AttemptId,
    /// Exact increment.
    pub increment: IncrementId,
    /// Claimed owner.
    pub owner: ActorId,
    /// Baseline when claimed.
    pub baseline: ContractRevision,
    /// Mission generation when claimed.
    pub generation: ContractRevision,
    /// Current metadata disposition.
    pub status: AttemptStatus,
    /// Original/reconciliation reference.
    pub basis: SourceReference,
}
/// A plan changes ordering only, never acceptance criteria, permissions or scope.
#[derive(Clone, PartialEq, Eq)]
pub struct Plan {
    /// Monotonic plan revision, independent of baseline revision.
    pub revision: ContractRevision,
    /// Exact mission ordering; earlier entries have owner-declared precedence.
    pub order: Vec<TaskId>,
    /// Reason for the ordering, without universal numerical weights.
    pub basis: SourceReference,
}
/// Validated program projection; the caller retains its ordered source inputs.
#[derive(Clone)]
pub struct ProgramState {
    definitions: Vec<ProgramDefinition>,
    readiness: ReadinessProjection,
    attempts: Vec<Attempt>,
    plan: Option<Plan>,
}
impl ProgramState {
    /// Construct the first accepted baseline. No mission is implicitly accepted.
    pub fn new(definition: ProgramDefinition) -> Result<Self, ProgramError> {
        if definition.predecessor.is_some() {
            return Err(ProgramError::Structure);
        }
        let readiness = ReadinessProjection::new(definition.graph.clone())?;
        Ok(Self {
            definitions: vec![definition],
            readiness,
            attempts: vec![],
            plan: None,
        })
    }
    /// Current immutable baseline.
    pub fn definition(&self) -> &ProgramDefinition {
        self.definitions.last().expect("initial baseline")
    }
    /// Full immutable baseline history.
    pub fn definitions(&self) -> &[ProgramDefinition] {
        &self.definitions
    }
    /// One G1 projection of canonical graph inputs.
    pub fn readiness(&self) -> &ReadinessProjection {
        &self.readiness
    }
    /// All metadata attempts, including terminal and uncertain attempts.
    pub fn attempts(&self) -> &[Attempt] {
        &self.attempts
    }
    /// Current ordering proposal.
    pub fn plan(&self) -> Option<&Plan> {
        self.plan.as_ref()
    }
    /// Stage 1 always denies execution authority.
    pub fn authority(&self) -> ExecutionAuthority {
        ExecutionAuthority::Unavailable
    }
    /// Change only a complete mission ordering with exact next plan revision.
    pub fn set_plan(&mut self, grant: &CoordinationGrant, plan: Plan) -> Result<(), ProgramError> {
        grant.check(self.definition(), CoordinationPermission::Plan, &plan.order)?;
        let next = self
            .plan
            .as_ref()
            .map_or(Some(1), |p| p.revision.get().checked_add(1));
        if next != Some(plan.revision.get()) {
            return Err(ProgramError::Stale);
        }
        let ids: BTreeSet<_> = plan.order.iter().map(TaskId::as_str).collect();
        if ids.len() != plan.order.len()
            || ids
                != self
                    .definition()
                    .graph
                    .missions()
                    .iter()
                    .map(|m| m.id().as_str())
                    .collect()
        {
            return Err(ProgramError::Structure);
        }
        self.plan = Some(plan);
        Ok(())
    }
    /// Apply a non-effectful graph notification under exact metadata scope.
    /// Review callers must additionally use Evidence's support-admission boundary.
    pub fn notify(
        &mut self,
        grant: &CoordinationGrant,
        input: Notification,
    ) -> Result<TransitionReport, ProgramError> {
        if input.id().as_str().starts_with("program.") {
            return Err(ProgramError::IdentityConflict);
        }
        let (permission, missions) = match input.event() {
            Event::Signal { mission, .. } => (CoordinationPermission::Plan, vec![mission.clone()]),
            Event::Artifact(r) => (
                CoordinationPermission::Plan,
                vec![r.producer().mission().clone()],
            ),
            Event::Review(r) => {
                let consumer = self
                    .definition()
                    .graph
                    .missions()
                    .iter()
                    .find(|m| m.id() == r.scopes()[1].mission())
                    .ok_or(ProgramError::Missing)?;
                if consumer.kind() != ConsumerKind::Preview {
                    return Err(ProgramError::EffectUnavailable);
                }
                (
                    CoordinationPermission::Review,
                    r.scopes().iter().map(|s| s.mission().clone()).collect(),
                )
            }
            Event::Invalidate { roots, .. } => (
                CoordinationPermission::Review,
                roots.iter().map(|s| s.mission().clone()).collect(),
            ),
            Event::Preconditions { scope, .. } => {
                (CoordinationPermission::Plan, vec![scope.mission().clone()])
            }
            Event::Capacity { .. } => (
                CoordinationPermission::Plan,
                self.definition()
                    .graph
                    .missions()
                    .iter()
                    .map(|m| m.id().clone())
                    .collect(),
            ),
            // Mission acceptance is not a worker metadata report.
            Event::Status { .. } | Event::Unsupported(_) => {
                return Err(ProgramError::EffectUnavailable);
            }
        };
        grant.check(self.definition(), permission, &missions)?;
        let mut projection = self.readiness.clone();
        let report = projection.apply(input)?;
        grant.check(self.definition(), permission, report.changed_missions())?;
        self.readiness = projection;
        Ok(report)
    }
    /// Claim one increment and its declared exclusive resource scopes atomically.
    pub fn claim(
        &mut self,
        grant: &CoordinationGrant,
        attempt: Attempt,
    ) -> Result<(), ProgramError> {
        let increment = self
            .definition()
            .increments
            .iter()
            .find(|i| i.id == attempt.increment)
            .ok_or(ProgramError::Missing)?;
        grant.check(
            self.definition(),
            CoordinationPermission::Claim,
            std::slice::from_ref(&increment.mission),
        )?;
        if attempt.owner != grant.actor
            || attempt.baseline != self.definition().revision
            || attempt.generation != self.readiness.scope(&increment.mission)?.generation()
            || attempt.status != AttemptStatus::Claimed
        {
            return Err(ProgramError::Stale);
        }
        if self.attempts.iter().any(|a| a.id == attempt.id) {
            return Err(ProgramError::IdentityConflict);
        }
        if self.attempts.len() >= self.definition().limits.entries {
            return Err(ProgramError::Capacity);
        }
        for a in self.attempts.iter().filter(|a| {
            matches!(
                a.status,
                AttemptStatus::Claimed | AttemptStatus::Unknown | AttemptStatus::Reported
            )
        }) {
            // Old baseline responsibility remains held until explicit reconciliation.
            let old = self
                .definitions
                .iter()
                .find(|d| d.revision == a.baseline)
                .and_then(|d| d.increments.iter().find(|i| i.id == a.increment))
                .ok_or(ProgramError::Structure)?;
            if old.id == increment.id
                || old.mission == increment.mission
                || old
                    .resources
                    .iter()
                    .any(|r| increment.resources.contains(r))
            {
                return Err(ProgramError::Claimed);
            }
        }
        let scope = self.readiness.scope(&increment.mission)?;
        self.readiness.apply(Notification::new(
            NotificationId::new(&format!("program.claim.{}", self.attempts.len()))?,
            self.definition().graph.reference().clone(),
            Event::Status {
                scope,
                status: MissionStatus::Active,
                source: attempt.basis.clone(),
            },
        )?)?;
        self.attempts.push(attempt);
        Ok(())
    }
    /// Retain a report/uncertainty/cancellation. Unknown can only be explicitly
    /// reconciled to reported/cancelled; no completion or external-effect inference.
    pub fn reconcile_attempt(
        &mut self,
        grant: &CoordinationGrant,
        id: &AttemptId,
        generation: ContractRevision,
        status: AttemptStatus,
        basis: SourceReference,
    ) -> Result<(), ProgramError> {
        let at = self
            .attempts
            .iter()
            .position(|a| &a.id == id)
            .ok_or(ProgramError::Missing)?;
        let a = &self.attempts[at];
        let original = self
            .definitions
            .iter()
            .find(|d| d.revision == a.baseline)
            .ok_or(ProgramError::Structure)?;
        let increment = original
            .increments
            .iter()
            .find(|i| i.id == a.increment)
            .ok_or(ProgramError::Structure)?;
        grant.check(
            self.definition(),
            CoordinationPermission::Claim,
            std::slice::from_ref(&increment.mission),
        )?;
        if a.owner != grant.actor || a.generation != generation {
            return Err(ProgramError::Stale);
        }
        // Stale work may be cancelled to release metadata ownership, never reported as current.
        if status != AttemptStatus::Cancelled
            && (a.baseline != self.definition().revision
                || self.readiness.scope(&increment.mission)?.generation() != generation)
        {
            return Err(ProgramError::Stale);
        }
        if !matches!(
            (a.status, status),
            (
                AttemptStatus::Claimed,
                AttemptStatus::Unknown | AttemptStatus::Reported | AttemptStatus::Cancelled
            ) | (
                AttemptStatus::Unknown,
                AttemptStatus::Reported | AttemptStatus::Cancelled
            ) | (AttemptStatus::Reported, AttemptStatus::Cancelled)
        ) {
            return Err(ProgramError::Transition);
        }
        self.attempts[at].status = status;
        self.attempts[at].basis = basis;
        Ok(())
    }
    /// Acknowledged handoff closes the old attempt and claims a distinct new one.
    pub fn handoff(
        &mut self,
        from: &CoordinationGrant,
        to: &CoordinationGrant,
        old: &AttemptId,
        new: Attempt,
    ) -> Result<(), ProgramError> {
        let mut draft = self.clone();
        let a = draft
            .attempts
            .iter()
            .find(|a| &a.id == old)
            .ok_or(ProgramError::Missing)?
            .clone();
        if a.status != AttemptStatus::Claimed || a.increment != new.increment {
            return Err(ProgramError::Transition);
        }
        draft.reconcile_attempt(
            from,
            old,
            a.generation,
            AttemptStatus::Cancelled,
            new.basis.clone(),
        )?;
        draft.claim(to, new)?;
        draft
            .attempts
            .iter_mut()
            .find(|a| &a.id == old)
            .expect("existing attempt")
            .status = AttemptStatus::HandedOver;
        *self = draft;
        Ok(())
    }
    /// Amend an owner-accepted baseline. Only unchanged connected components may
    /// retain prior G1 inputs; changed contracts invalidate their causal closure.
    /// Returns exact affected mission identities for inspection and requalification.
    pub fn amend(
        &mut self,
        grant: &CoordinationGrant,
        next: ProgramDefinition,
    ) -> Result<Vec<TaskId>, ProgramError> {
        let old = self.definition();
        let all: Vec<_> = old
            .graph
            .missions()
            .iter()
            .map(|m| m.id().clone())
            .collect();
        grant.check(old, CoordinationPermission::Amend, &all)?;
        if next.id != old.id
            || next.predecessor != Some(old.revision)
            || next.limits != old.limits
            || next.graph.reference().id() != old.graph.reference().id()
            || next.graph.reference().revision().get()
                != old
                    .graph
                    .reference()
                    .revision()
                    .get()
                    .checked_add(1)
                    .ok_or(ProgramError::Capacity)?
        {
            return Err(ProgramError::Stale);
        }
        if self.definitions.len() >= old.limits.revisions {
            return Err(ProgramError::Capacity);
        }
        // Stage 1 amendments preserve membership; owner-authorized scope expansion
        // requires a separately constructed program, never an incidental plan edit.
        if old.workstreams != next.workstreams
            || old.increments.len() != next.increments.len()
            || old.increments.iter().any(|i| {
                !next
                    .increments
                    .iter()
                    .any(|n| n.id == i.id && n.mission == i.mission && n.resources == i.resources)
            })
        {
            return Err(ProgramError::Structure);
        }
        let mut affected: BTreeSet<String> = BTreeSet::new();
        if old.outcome != next.outcome {
            affected.extend(all.iter().map(|m| m.as_str().to_owned()));
        }
        for m in old.graph.missions() {
            if next.graph.missions().iter().find(|n| n.id() == m.id()) != Some(m) {
                affected.insert(m.id().as_str().to_owned());
            }
        }
        if all.len() != next.graph.missions().len() {
            return Err(ProgramError::Structure);
        }
        for e in old.graph.edges().iter().chain(next.graph.edges()) {
            let other = if old.graph.edges().contains(e) {
                next.graph.edges()
            } else {
                old.graph.edges()
            };
            if !other.contains(e) {
                affected.insert(e.consumer().as_str().to_owned());
            }
        }
        loop {
            let before = affected.len();
            for e in old.graph.edges().iter().chain(next.graph.edges()) {
                if affected.contains(e.producer().as_str()) {
                    affected.insert(e.consumer().as_str().to_owned());
                }
            }
            if before == affected.len() {
                break;
            }
        }
        if !affected.is_empty()
            && old
                .graph
                .missions()
                .iter()
                .chain(next.graph.missions())
                .any(|m| m.coverage() == DependencyCoverage::Unknown)
        {
            affected.extend(all.iter().map(|m| m.as_str().to_owned()));
        }
        let mut projection = ReadinessProjection::new(next.graph.clone())?;
        for input in self.readiness.notifications() {
            let keep = match input.event() {
                Event::Signal { mission, .. } => !affected.contains(mission.as_str()),
                Event::Artifact(r) => {
                    !affected.contains(r.producer().mission().as_str())
                        && next.graph.edges().iter().any(|e| {
                            e.producer() == r.producer().mission()
                                && e.requirement().artifact() == r.artifact()
                                && e.requirement().kind() == r.kind()
                        })
                }
                Event::Review(r) => r
                    .scopes()
                    .iter()
                    .all(|s| !affected.contains(s.mission().as_str())),
                Event::Preconditions { scope, .. } | Event::Status { scope, .. } => {
                    !affected.contains(scope.mission().as_str())
                }
                Event::Invalidate { roots, .. } => roots
                    .iter()
                    .all(|s| !affected.contains(s.mission().as_str())),
                Event::Capacity { .. } => true,
                _ => false,
            };
            if keep {
                projection.apply(Notification::new(
                    input.id().clone(),
                    next.graph.reference().clone(),
                    input.event().clone(),
                )?)?;
            }
        }
        let result = all
            .into_iter()
            .filter(|m| affected.contains(m.as_str()))
            .collect();
        self.definitions.push(next);
        self.readiness = projection;
        Ok(result)
    }
}
