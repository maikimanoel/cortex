//! Finite scheduling-to-coordination preparation, without admission or dispatch.
//!
//! Preparation retains the canonical scheduling decisions and validates explicitly
//! supplied Claim/Package operations on a draft. It grants no execution authority,
//! reserves no durable resources, and is not an atomic durable transaction.
use cortex_core::{
    TaskId,
    binding::CandidateRequirement,
    program::{
        AttemptStatus, CoordinationPermission, ProgramError,
        context::{self, PackageRequest},
        executor::PreparedInput,
        operational::OperationalError,
        scheduling::{Proposal, SchedulingDecision},
    },
};
use cortex_evidence::program::{Command, CoordinationKernel, KernelError, Operation};
use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
};

/// Exact task inputs. Absence explicitly discloses unavailable coordination authority.
#[derive(Clone, PartialEq, Eq)]
pub struct TaskOperations {
    /// Task appearing exactly once in the supplied scheduling proposals.
    pub task: TaskId,
    /// Exact expected candidate, including repository and worktree requirements.
    pub candidate: CandidateRequirement,
    /// Explicit Claim followed by Package, or no supplied coordination authority.
    pub operations: Option<[Operation; 2]>,
}

/// A preparation limitation supplementary to the unchanged scheduling decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreparationHold {
    /// The caller supplied no Claim/Package operations with their required grants.
    MissingAuthority,
}

/// Canonical scheduling output together with explicit coordination limitations.
#[derive(Clone, PartialEq, Eq)]
pub struct PreparationDecision {
    /// Unmodified output from the existing operational scheduling preview.
    pub scheduling: SchedulingDecision,
    /// Additional reason why a scheduling proposal was not prepared.
    pub coordination_hold: Option<PreparationHold>,
}

/// A bounded proposal. Applying it requires separate authoritative admission.
#[derive(Clone, PartialEq, Eq)]
pub struct CoordinationPreparation {
    revision: u64,
    decisions: Vec<PreparationDecision>,
    operations: Vec<Operation>,
}
impl fmt::Debug for CoordinationPreparation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CoordinationPreparation")
            .field("revision", &self.revision)
            .field("decision_count", &self.decisions.len())
            .field("operation_count", &self.operations.len())
            .finish()
    }
}
impl CoordinationPreparation {
    /// Exact snapshot revision used for this proposal.
    pub fn revision(&self) -> u64 {
        self.revision
    }
    /// Every supplied scheduling proposal, including all held reasons.
    pub fn decisions(&self) -> &[PreparationDecision] {
        &self.decisions
    }
    /// Exact selected operations and their unchanged expected predecessors.
    pub fn operations(&self) -> &[Operation] {
        &self.operations
    }
    /// Transfer explicit operations to a caller; this performs no admission.
    pub fn into_operations(self) -> Vec<Operation> {
        self.operations
    }
}

/// Non-disclosing validation refusal preserving the canonical cause.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreparationError {
    /// Duplicate, absent, foreign or inconsistent task correspondence.
    Mapping,
    /// The operation does not belong to the selected preparation path.
    UnsupportedCommand,
    /// A Core correspondence or bounded-input guard refused the request.
    Core(ProgramError),
    /// The canonical operational resource projection refused scheduling.
    Operational(OperationalError),
    /// Canonical admission refused the in-memory draft.
    Kernel(KernelError),
}

/// Exact executor input proposed against one immutable kernel revision.
///
/// This owns the original canonical operation and the input produced by the
/// existing kernel guards. It has no transport, submission or resource grant.
/// Durable admission must still check the original predecessor and may fail.
#[derive(Clone, PartialEq, Eq)]
pub struct ExecutorPreparation {
    revision: u64,
    operation: Operation,
    input: PreparedInput,
}
impl fmt::Debug for ExecutorPreparation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ExecutorPreparation")
            .field("revision", &self.revision)
            .field("input_bytes", &self.input.bytes().len())
            .finish_non_exhaustive()
    }
}
impl ExecutorPreparation {
    /// Exact accepted-operation count inspected before preparing this input.
    pub fn revision(&self) -> u64 {
        self.revision
    }
    /// Unchanged operation, grant and expected predecessor; never a receipt.
    pub fn operation(&self) -> &Operation {
        &self.operation
    }
    /// Canonical complete input and its exact descriptor/control requirements.
    pub fn input(&self) -> &PreparedInput {
        &self.input
    }
    /// Transfer the operation for separately explicit authoritative admission.
    pub fn into_operation(self) -> Operation {
        self.operation
    }
}

/// Prepare an explicit executor-input operation for an exact retained task package.
///
/// Callers first admit their selected Claim/Package operations through the existing
/// authoritative path. This consumer then checks the exact current candidate and
/// package identity and invokes the canonical kernel guards on an isolated draft.
/// Capability holds, changed descriptor identities and package staleness retain
/// their existing typed causes. It does not discover an executable or qualify a host.
///
/// Only a fresh `PrepareExecutorInput` operation is accepted here. Historical exact
/// replay remains the journal's responsibility; this function never rebases or
/// retries an operation. The caller's kernel, attempts, reservations and observations
/// are unchanged on both success and error. Input preparation does not authorize a
/// later process or associate an operational effect with an ownership attempt.
pub fn prepare_executor_input(
    kernel: &CoordinationKernel,
    expected_revision: u64,
    task: &TaskId,
    candidate: &CandidateRequirement,
    operation: Operation,
) -> Result<ExecutorPreparation, PreparationError> {
    if expected_revision != kernel.revision() || operation.expected != expected_revision {
        return Err(ProgramError::Stale.into());
    }
    if kernel.operations().iter().any(|old| old.id == operation.id) {
        return Err(ProgramError::IdentityConflict.into());
    }
    let Command::PrepareExecutorInput(request) = &operation.command else {
        return Err(PreparationError::UnsupportedCommand);
    };
    let limits = kernel.state().definition().limits();
    if request.descriptor.observations().len() > limits.entries
        || request.requirements.required().len() > limits.entries
        || request.max_bytes == 0
        || request.max_bytes > limits.bytes
    {
        return Err(ProgramError::Capacity.into());
    }
    let package = kernel
        .packages()
        .iter()
        .find(|p| p.request().id == request.package && p.request().revision == request.revision)
        .ok_or(ProgramError::Missing)?;
    if &package.request().task != task {
        return Err(PreparationError::Mapping);
    }
    let scope = kernel
        .state()
        .readiness()
        .scope(task)
        .map_err(ProgramError::from)?;
    if scope.context().contract().candidate() != candidate {
        return Err(ProgramError::Stale.into());
    }
    let input = kernel.preview_executor_input(&operation)?;
    Ok(ExecutorPreparation {
        revision: expected_revision,
        operation,
        input,
    })
}
impl fmt::Display for PreparationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Mapping => f.write_str("preparation: invalid task correspondence"),
            Self::UnsupportedCommand => f.write_str("preparation: unsupported command"),
            Self::Core(e) => write!(f, "preparation: {e}"),
            Self::Operational(e) => write!(f, "preparation: {e}"),
            Self::Kernel(e) => write!(f, "preparation: {e}"),
        }
    }
}
impl Error for PreparationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Core(e) => Some(e),
            Self::Operational(e) => Some(e),
            Self::Kernel(e) => Some(e),
            Self::Mapping | Self::UnsupportedCommand => None,
        }
    }
}
impl From<ProgramError> for PreparationError {
    fn from(e: ProgramError) -> Self {
        Self::Core(e)
    }
}
impl From<OperationalError> for PreparationError {
    fn from(e: OperationalError) -> Self {
        Self::Operational(e)
    }
}
impl From<KernelError> for PreparationError {
    fn from(e: KernelError) -> Self {
        Self::Kernel(e)
    }
}

// Bound variable package text before cloning a caller-owned request. Canonical
// assembly subsequently checks complete framed bytes and source correspondence.
fn bound_request(
    request: &PackageRequest,
    entries: usize,
    bytes: usize,
) -> Result<(), ProgramError> {
    if request.sources.len() > entries || request.methods.len() > entries {
        return Err(ProgramError::Capacity);
    }
    let text = [
        &request.omissions,
        &request.deliverable,
        &request.stop_rules,
    ]
    .into_iter()
    .chain(request.sources.iter().map(|s| &s.text))
    .chain(
        request
            .methods
            .iter()
            .flat_map(|m| [&m.applicability, &m.exceptions]),
    );
    let mut total = 0usize;
    for value in text {
        total = total
            .checked_add(value.len())
            .ok_or(ProgramError::Capacity)?;
        if total > bytes {
            return Err(ProgramError::Capacity);
        }
    }
    Ok(())
}

/// Prepare explicitly supplied operations against one exact kernel snapshot.
///
/// Every supplied pair, including a held task's pair, must expect the current
/// cursor and cursor+1 in canonical scheduling order. Only selected pairs advance
/// the cursor by two. No predecessor is rebased. Missing authority remains a
/// supplemental hold; scheduling's speculative resource allocation is unchanged.
///
/// One outer draft is created lazily. Its authoritative `admit` still performs
/// its own existing transactional clones. No store, process or external effect
/// is accessible here. Later durable admission may commit a prefix: callers must
/// retain it and stop on failure, without automatically retrying Indeterminate.
pub fn prepare(
    kernel: &CoordinationKernel,
    expected_revision: u64,
    proposals: &[Proposal],
    mappings: &[TaskOperations],
) -> Result<CoordinationPreparation, PreparationError> {
    if expected_revision != kernel.revision() {
        return Err(ProgramError::Stale.into());
    }
    let limits = kernel.state().definition().limits();
    if proposals.len() > limits.entries || mappings.len() > limits.entries {
        return Err(ProgramError::Capacity.into());
    }
    let mut by_task = BTreeMap::new();
    let mut operation_ids = BTreeSet::new();
    let mut attempt_ids = BTreeSet::new();
    let mut package_ids = BTreeSet::new();
    let mut grants = BTreeMap::new();
    for operation in kernel.operations() {
        grants.insert(operation.grant.id.as_str(), &operation.grant);
        if let Command::Handoff { recipient, .. } = &operation.command {
            grants.insert(recipient.id.as_str(), recipient);
        }
    }
    for mapping in mappings {
        if by_task.insert(mapping.task.as_str(), mapping).is_some() {
            return Err(PreparationError::Mapping);
        }
        let scope = kernel
            .state()
            .readiness()
            .scope(&mapping.task)
            .map_err(ProgramError::from)?;
        if mapping.candidate != *scope.context().contract().candidate() {
            return Err(ProgramError::Stale.into());
        }
        if let Some([claim, package]) = &mapping.operations {
            let (Command::Claim(attempt), Command::Package(request)) =
                (&claim.command, &package.command)
            else {
                return Err(PreparationError::UnsupportedCommand);
            };
            for operation in [claim, package] {
                if !operation_ids.insert(operation.id.as_str())
                    || kernel.operations().iter().any(|old| old.id == operation.id)
                {
                    return Err(ProgramError::IdentityConflict.into());
                }
                if let Some(old) = grants.insert(operation.grant.id.as_str(), &operation.grant)
                    && old != &operation.grant
                {
                    return Err(ProgramError::IdentityConflict.into());
                }
            }
            if operation_ids.len() > limits.entries {
                return Err(ProgramError::Capacity.into());
            }
            let increment = kernel
                .state()
                .definition()
                .increments()
                .iter()
                .find(|increment| increment.id == attempt.increment)
                .ok_or(PreparationError::Mapping)?;
            if increment.task != mapping.task || request.task != mapping.task {
                return Err(PreparationError::Mapping);
            }
            claim.grant.check(
                kernel.state().definition(),
                CoordinationPermission::Claim,
                std::slice::from_ref(&mapping.task),
            )?;
            package.grant.check(
                kernel.state().definition(),
                CoordinationPermission::Plan,
                std::slice::from_ref(&mapping.task),
            )?;
            if attempt.owner != claim.grant.actor
                || attempt.status != AttemptStatus::Claimed
                || attempt.baseline != kernel.state().definition().revision()
                || request.baseline != attempt.baseline
                || attempt.generation != scope.generation()
                || request.generation != attempt.generation
            {
                return Err(ProgramError::Stale.into());
            }
            if !attempt_ids.insert(&attempt.id)
                || kernel
                    .state()
                    .attempts()
                    .iter()
                    .any(|old| old.id == attempt.id)
                || !package_ids.insert(&request.id)
                || kernel
                    .packages()
                    .iter()
                    .any(|old| old.request().id == request.id)
            {
                return Err(ProgramError::IdentityConflict.into());
            }
            bound_request(request, limits.entries, limits.bytes)?;
        }
    }
    let proposal_tasks: BTreeSet<_> = proposals.iter().map(|p| p.task.as_str()).collect();
    if proposal_tasks.len() != proposals.len()
        || proposal_tasks.len() != by_task.len()
        || proposal_tasks
            .iter()
            .any(|task| !by_task.contains_key(task))
    {
        return Err(PreparationError::Mapping);
    }
    let scheduling = kernel
        .fixture_effects()
        .preview_schedule(kernel.state(), proposals)?;
    let mut result = CoordinationPreparation {
        revision: expected_revision,
        decisions: Vec::with_capacity(scheduling.len()),
        operations: Vec::new(),
    };
    let mut draft = None;
    let mut cursor = expected_revision;
    for decision in scheduling {
        let mapping = by_task
            .get(decision.task.as_str())
            .ok_or(PreparationError::Mapping)?;
        let hold = if let Some([claim, package]) = &mapping.operations {
            let next = cursor.checked_add(1).ok_or(ProgramError::Capacity)?;
            if claim.expected != cursor || package.expected != next {
                return Err(ProgramError::Stale.into());
            }
            if decision.proposed {
                let draft = draft.get_or_insert_with(|| kernel.clone());
                draft.admit(claim.clone())?;
                draft.admit(package.clone())?;
                result.operations.extend([claim.clone(), package.clone()]);
                cursor = next.checked_add(1).ok_or(ProgramError::Capacity)?;
            } else if let Command::Package(request) = &package.command {
                context::assemble(kernel.state(), request.clone())?;
            }
            None
        } else {
            Some(PreparationHold::MissingAuthority)
        };
        result.decisions.push(PreparationDecision {
            scheduling: decision,
            coordination_hold: hold,
        });
    }
    Ok(result)
}
