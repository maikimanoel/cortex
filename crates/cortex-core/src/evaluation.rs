//! Frozen, caller-declared evaluation inputs under CAP-EVL-001/002.
//!
//! These contracts neither sample source trees nor grant execution, independent
//! acceptance, protected-oracle access or comparison-publication permission.
use crate::{
    CandidateId, ContractError, TaskId,
    binding::{
        CandidateBindingContract, CandidateRequirement, ContractRevision, WorktreeRequirement,
    },
    program::{ProgramError, scheduling::Demand},
    temporal::SourceReference,
};
use std::{error::Error, fmt};

/// Rejected evaluation contract, without private input in diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvaluationError {
    /// Existing identity/text validation failed.
    Contract(ContractError),
    /// Existing resource correspondence validation failed.
    Resource(ProgramError),
    /// A logical count exceeds the explicit finite bounds.
    Limit,
    /// Missing, duplicate or inconsistent case/trial/arm membership.
    Population,
    /// Inconsistent configuration or source identity.
    Configuration,
}
impl fmt::Display for EvaluationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "evaluation contract: {self:?}")
    }
}
impl Error for EvaluationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Contract(e) => Some(e),
            Self::Resource(e) => Some(e),
            _ => None,
        }
    }
}
macro_rules! identity {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, PartialEq, Eq)]
        pub struct $name(TaskId);
        impl $name {
            /// Validate existing identifier syntax; this creates no task or authority.
            pub fn new(value: &str) -> Result<Self, EvaluationError> {
                TaskId::new(value)
                    .map(Self)
                    .map_err(EvaluationError::Contract)
            }
            /// Explicit access to caller-assigned identity text.
            pub fn as_str(&self) -> &str {
                self.0.as_str()
            }
        }
        impl Ord for $name {
            fn cmp(&self, other: &Self) -> std::cmp::Ordering {
                self.as_str().cmp(other.as_str())
            }
        }
        impl PartialOrd for $name {
            fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
                Some(self.cmp(other))
            }
        }
        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(concat!(stringify!($name), "([redacted])"))
            }
        }
    };
}
identity!(
    CaseId,
    "Evaluation case identity, separate from its engineering task."
);
identity!(
    TrialId,
    "One predeclared case/strategy/repetition, not an independent attempt."
);
identity!(
    AttemptId,
    "One retained evaluation attempt; retries have distinct identities."
);

/// A distinct asserted Git tree object ID. Syntax does not establish existence,
/// type, clean source correspondence or that the caller actually inspected Git.
#[derive(Clone, PartialEq, Eq)]
pub struct SourceTreeId(CandidateId);
impl SourceTreeId {
    /// Validate full lowercase Git object-ID syntax without resolving an object.
    pub fn new(value: &str) -> Result<Self, EvaluationError> {
        CandidateId::from_git_commit(value)
            .map(Self)
            .map_err(EvaluationError::Contract)
    }
    /// Full asserted tree ID, distinct from commit and task identities.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}
impl fmt::Debug for SourceTreeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SourceTreeId([redacted])")
    }
}
/// Exact source requirement and separate tree identity, supplied by an adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceIdentity {
    candidate: CandidateRequirement,
    tree: SourceTreeId,
}
impl SourceIdentity {
    /// This first profile requires clean committed source and matching Git hash
    /// widths. Actual object/tree correspondence remains the adapter's obligation.
    pub fn new(
        candidate: CandidateRequirement,
        tree: SourceTreeId,
    ) -> Result<Self, EvaluationError> {
        if candidate.worktree() != WorktreeRequirement::Clean
            || candidate.commit().as_str().len() != tree.as_str().len()
        {
            return Err(EvaluationError::Configuration);
        }
        Ok(Self { candidate, tree })
    }
    /// Exact repository/worktree/commit requirement.
    pub fn candidate(&self) -> &CandidateRequirement {
        &self.candidate
    }
    /// Separate asserted source tree identity.
    pub fn tree(&self) -> &SourceTreeId {
        &self.tree
    }
}
/// Proposed comparison arms. A name does not supply or qualify its runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ExecutionStrategy {
    /// One sequential engineering execution path.
    SequentialExecution,
    /// Multiple paths coordinated without GPE.
    DirectParallelExecution,
    /// Cortex-governed admission, dependencies, Evidence and supervision.
    GovernedParallelEngineering,
}
/// Actual execution scope allowed by a declared development contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionProfile {
    /// Programmatically supplied synthetic cases; no execution claim.
    Synthetic,
    /// Finite locally owned provider-free development fixture.
    ProviderFreeFixture,
    /// Future provider execution; this foundation refuses performed observations.
    LiveProvider,
}
/// Model configuration is independent of harness/strategy treatment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelConfiguration {
    /// No model participates in this explicit provider-free profile.
    NotApplicable,
    /// Exact proposed model/settings reference; no invocation permission.
    Configured(SourceReference),
    /// Configuration was not established; it cannot support comparison.
    Unestablished,
}
/// Representative task family; no corpus, sample size or score is selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkloadFamily {
    /// Tasks with declared independent responsibilities.
    Independent,
    /// Intermediate artifacts and material dependencies.
    DependencyHeavy,
    /// Interface or integration conflicts.
    IntegrationConflict,
    /// Interrupted work, recovery and reconciliation.
    InterruptionRecovery,
    /// An explicitly selected real roadmap task.
    Roadmap,
}
/// Case disclosure classification, not an implemented access-control mechanism.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaseAccess {
    /// Public synthetic development case.
    Development,
    /// Only an external protected-case reference is retained here.
    ReservedReference,
}
/// Untrusted case input, frozen and validated by EvaluationSpecification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvaluationCase {
    /// Exact case identity.
    pub id: CaseId,
    /// Positive immutable case revision.
    pub revision: ContractRevision,
    /// Full engineering task/criterion revision and starting candidate requirement.
    pub contract: CandidateBindingContract,
    /// Exact starting source and tree assertion.
    pub source: SourceIdentity,
    /// Separate oracle identity/version; unified oracle prose alone has no such ID.
    pub oracle: SourceReference,
    /// Declared workload family.
    pub family: WorkloadFamily,
    /// Disclosure classification; protected case/oracle content is not stored.
    pub access: CaseAccess,
}
/// Exact declared controls. Harness and strategy are the intended treatment;
/// model, tools, environment, information and resource differences are confounders.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunConfiguration {
    /// Immutable configuration reference/version.
    pub reference: SourceReference,
    /// Actual permitted profile; this is data, not a grant.
    pub profile: ExecutionProfile,
    /// Exact model/settings or explicit absence/uncertainty.
    pub model: ModelConfiguration,
    /// Exact ordered participating tool versions, not executable discovery.
    pub tools: Vec<SourceReference>,
    /// Harness implementation/configuration version, the declared treatment.
    pub harness: SourceReference,
    /// Host/runtime environment version and its declared limits.
    pub environment: SourceReference,
    /// Exact input/context/information-access configuration.
    pub information: SourceReference,
    /// Explicit independent resource dimensions; unknown amounts remain unknown.
    /// These are declared budgets, not observed usage or physical enforcement.
    pub envelope: Vec<Demand>,
}
impl RunConfiguration {
    /// Whether all declared non-treatment controls match exactly. This establishes
    /// neither causal identification nor adequate experimental design.
    pub fn controls_match(&self, other: &Self) -> bool {
        self.profile == other.profile
            && self.model == other.model
            && self.tools == other.tools
            && self.environment == other.environment
            && self.information == other.information
            && self.envelope == other.envelope
    }
    fn validate(&self) -> Result<(), EvaluationError> {
        if self.tools.len() > 16 || self.envelope.is_empty() || self.envelope.len() > 10 {
            return Err(EvaluationError::Limit);
        }
        if self
            .tools
            .iter()
            .enumerate()
            .any(|(i, t)| self.tools[..i].contains(t))
            || self
                .envelope
                .iter()
                .enumerate()
                .any(|(i, d)| self.envelope[..i].iter().any(|p| p.resource == d.resource))
        {
            return Err(EvaluationError::Configuration);
        }
        for d in &self.envelope {
            d.resource
                .validate_accounting_scope()
                .map_err(EvaluationError::Resource)?;
            if d.resource.memory_basis_unestablished() {
                return Err(EvaluationError::Configuration);
            }
        }
        if self.profile == ExecutionProfile::ProviderFreeFixture
            && self.model != ModelConfiguration::NotApplicable
        {
            return Err(EvaluationError::Configuration);
        }
        Ok(())
    }
}
/// One frozen strategy/configuration definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComparisonArm {
    /// Proposed strategy, not a claim of runtime availability.
    pub strategy: ExecutionStrategy,
    /// Exact profile, treatment and matching controls.
    pub configuration: RunConfiguration,
}
/// Predeclared trial population entry. Repetitions do not establish independence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrialDefinition {
    /// Unique trial identity.
    pub id: TrialId,
    /// Selected case.
    pub case: CaseId,
    /// Selected comparison arm.
    pub strategy: ExecutionStrategy,
    /// Positive matching repetition identity, not an independence assertion.
    pub repetition: u32,
}
/// Finite logical count bounds, not an allocator, host or operational budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EvaluationLimits {
    /// Maximum retained cases (at most 32).
    pub cases: usize,
    /// Maximum declared trials (at most 256).
    pub trials: usize,
    /// Maximum retained attempts (at most 256).
    pub attempts: usize,
}
impl EvaluationLimits {
    /// Validate positive bounded counts; source/profile text keeps existing limits.
    pub fn new(cases: usize, trials: usize, attempts: usize) -> Result<Self, EvaluationError> {
        if cases == 0
            || cases > 32
            || trials == 0
            || trials > 256
            || attempts == 0
            || attempts > 256
        {
            return Err(EvaluationError::Limit);
        }
        Ok(Self {
            cases,
            trials,
            attempts,
        })
    }
}
/// Frozen complete specification. No mutation, case filtering or attempt selection
/// is offered; a changed definition requires a distinct current-context examination.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvaluationSpecification {
    reference: SourceReference,
    cases: Vec<EvaluationCase>,
    arms: Vec<ComparisonArm>,
    trials: Vec<TrialDefinition>,
    limits: EvaluationLimits,
}
impl EvaluationSpecification {
    /// Validate references, exact membership and immutable count bounds. Unmatched
    /// arm populations remain representable but inspection marks them non-comparable.
    pub fn new(
        reference: SourceReference,
        cases: Vec<EvaluationCase>,
        arms: Vec<ComparisonArm>,
        trials: Vec<TrialDefinition>,
        limits: EvaluationLimits,
    ) -> Result<Self, EvaluationError> {
        let checked = EvaluationLimits::new(limits.cases, limits.trials, limits.attempts)?;
        if cases.is_empty()
            || cases.len() > checked.cases
            || trials.is_empty()
            || trials.len() > checked.trials
            || arms.is_empty()
            || arms.len() > 3
        {
            return Err(EvaluationError::Limit);
        }
        for (i, case) in cases.iter().enumerate() {
            if cases[..i]
                .iter()
                .any(|c| c.id == case.id || c.contract.task().id() == case.contract.task().id())
                || case.contract.candidate() != case.source.candidate()
            {
                return Err(EvaluationError::Population);
            }
        }
        for (i, arm) in arms.iter().enumerate() {
            arm.configuration.validate()?;
            if arms[..i].iter().any(|a| {
                a.strategy == arm.strategy
                    || (a.configuration.reference == arm.configuration.reference
                        && a.configuration != arm.configuration)
            }) {
                return Err(EvaluationError::Population);
            }
        }
        for (i, trial) in trials.iter().enumerate() {
            if trial.repetition == 0
                || !cases.iter().any(|c| c.id == trial.case)
                || !arms.iter().any(|a| a.strategy == trial.strategy)
                || trials[..i].iter().any(|t| {
                    t.id == trial.id
                        || (t.case == trial.case
                            && t.strategy == trial.strategy
                            && t.repetition == trial.repetition)
                })
            {
                return Err(EvaluationError::Population);
            }
        }
        Ok(Self {
            reference,
            cases,
            arms,
            trials,
            limits: checked,
        })
    }
    /// Exact specification identity/version; contents remain part of correspondence.
    pub fn reference(&self) -> &SourceReference {
        &self.reference
    }
    /// Complete retained case population.
    pub fn cases(&self) -> &[EvaluationCase] {
        &self.cases
    }
    /// All proposed arms, including unavailable runtimes.
    pub fn arms(&self) -> &[ComparisonArm] {
        &self.arms
    }
    /// Complete planned population, never only successful trials.
    pub fn trials(&self) -> &[TrialDefinition] {
        &self.trials
    }
    /// Caller-selected finite bounds.
    pub fn limits(&self) -> EvaluationLimits {
        self.limits
    }
}
