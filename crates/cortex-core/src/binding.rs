//! CAP-CORE-002: deterministic task/candidate correspondence within observation coverage.
//!
//! Producers supply observations, including their ordering and consistency claims.
//! This module validates their structure and compares them; it performs no I/O,
//! authenticates no producer, and grants neither execution authority nor assessment.

use crate::{CandidateId, TaskContract};
use std::{
    error::Error,
    fmt,
    path::{Component, Path, PathBuf},
};

/// Maximum changed paths retained from one inspection.
pub const MAX_OBSERVATION_ENTRIES: usize = 10_000;
/// Maximum path-byte total, or individual identity/branch field size.
pub const MAX_OBSERVATION_BYTES: usize = 4 * 1024 * 1024;

/// Structurally invalid binding input; diagnostics never repeat private values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BindingError {
    /// Contract revision zero is reserved as invalid.
    InvalidContractRevision,
    /// Producer scope or observation sequence is invalid.
    InvalidObservationId,
    /// Comparison profile or material is missing or malformed.
    InvalidComparisonBasis,
    /// A repository identity has a relative, empty, traversing or NUL-bearing path.
    InvalidRepositoryIdentity,
    /// A branch identity is empty or contains forbidden bytes.
    InvalidHead,
    /// A path or tracked entry is malformed or has no change.
    InvalidEntry,
    /// More than one change record names the same path.
    DuplicateEntry,
    /// Observation input exceeds a documented entry or byte bound.
    ObservationLimit,
    /// A task/revision pair was reused for different contract contents or requirements.
    ContractRevisionReused,
}

impl fmt::Display for BindingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidContractRevision => "contract revision must be nonzero",
            Self::InvalidObservationId => "observation needs a valid producer scope and positive sequence",
            Self::InvalidComparisonBasis => "comparison basis needs a bounded profile and nonempty material",
            Self::InvalidRepositoryIdentity => "repository identity requires absolute non-traversing paths without NUL bytes",
            Self::InvalidHead => "observation has an invalid branch identity",
            Self::InvalidEntry => "observation has an invalid relative path or change record",
            Self::DuplicateEntry => "observation has duplicate change paths",
            Self::ObservationLimit => "observation exceeds its entry or byte limit",
            Self::ContractRevisionReused => "task contract revision was reused with different contents or candidate requirements",
        })
    }
}
impl Error for BindingError {}

/// An explicit contract revision, independent of source revisions and format versions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContractRevision(u64);
impl ContractRevision {
    /// Accept any positive revision; equality, not numeric magnitude, establishes applicability.
    pub fn new(value: u64) -> Result<Self, BindingError> {
        if value == 0 {
            return Err(BindingError::InvalidContractRevision);
        }
        Ok(Self(value))
    }
    /// Read the caller-assigned revision.
    pub fn get(self) -> u64 {
        self.0
    }
}

/// A producer-local observation reference, not a content hash or authenticated timestamp.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservationId {
    scope: String,
    sequence: u64,
}
impl ObservationId {
    /// Scope is 1..=128 ASCII bytes, alphanumeric first, then alphanumeric/`.`/`_`/`-`.
    /// Producers must use one scope for an ordered capture series and never reuse a sequence.
    pub fn new(scope: &str, sequence: u64) -> Result<Self, BindingError> {
        if sequence == 0
            || scope.is_empty()
            || scope.len() > 128
            || !scope.as_bytes()[0].is_ascii_alphanumeric()
            || !scope
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
        {
            return Err(BindingError::InvalidObservationId);
        }
        Ok(Self {
            scope: scope.to_owned(),
            sequence,
        })
    }
    /// Read the producer's ordering scope.
    pub fn scope(&self) -> &str {
        &self.scope
    }
    /// Read the sequence within that scope.
    pub fn sequence(&self) -> u64 {
        self.sequence
    }
}

/// Local identity of a specific worktree and its metadata locations.
///
/// Construction checks syntax only. The producer must canonicalize and resolve the
/// actual paths. Copies/moves and linked worktrees have distinct local identities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositoryIdentity {
    root: PathBuf,
    git_dir: PathBuf,
    common_dir: PathBuf,
}
impl RepositoryIdentity {
    /// Validate absolute paths without accessing the filesystem.
    pub fn new(root: PathBuf, git_dir: PathBuf, common_dir: PathBuf) -> Result<Self, BindingError> {
        for path in [&root, &git_dir, &common_dir] {
            let bytes = path.as_os_str().as_encoded_bytes();
            if bytes.len() > MAX_OBSERVATION_BYTES {
                return Err(BindingError::ObservationLimit);
            }
            if !path.is_absolute()
                || bytes.contains(&0)
                || path
                    .components()
                    .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
            {
                return Err(BindingError::InvalidRepositoryIdentity);
            }
        }
        Ok(Self {
            root,
            git_dir,
            common_dir,
        })
    }
    /// Canonical working-tree location asserted by the producer.
    pub fn root(&self) -> &Path {
        &self.root
    }
    /// Canonical per-worktree Git directory asserted by the producer.
    pub fn git_dir(&self) -> &Path {
        &self.git_dir
    }
    /// Canonical common Git directory asserted by the producer.
    pub fn common_dir(&self) -> &Path {
        &self.common_dir
    }
}

/// Observed HEAD state. A missing observation differs from an observed unborn branch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Head {
    /// A named branch resolves to a commit.
    Attached {
        /// Unquoted branch-name bytes.
        branch: Vec<u8>,
        /// Locally resolved commit, as reported by the producer.
        commit: CandidateId,
    },
    /// HEAD resolves to a commit without a named branch.
    Detached {
        /// Locally resolved commit, as reported by the producer.
        commit: CandidateId,
    },
    /// A named branch has no commit.
    Unborn {
        /// Unquoted branch-name bytes.
        branch: Vec<u8>,
    },
    /// HEAD could not be established.
    Unknown,
}

/// A tracked-path change category, independently applicable to index and worktree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    /// Added path.
    Added,
    /// Modified contents.
    Modified,
    /// Deleted path.
    Deleted,
    /// Changed file type or mode category.
    TypeChanged,
}

/// A root-relative path and its observed change categories, never a content identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Entry {
    /// Tracked path; at least one side must contain a change.
    Tracked {
        /// Unquoted path bytes.
        path: Vec<u8>,
        /// Difference between HEAD and index.
        staged: Option<Change>,
        /// Difference between index and working tree.
        unstaged: Option<Change>,
    },
    /// Non-ignored untracked path.
    Untracked {
        /// Unquoted path bytes.
        path: Vec<u8>,
    },
}
impl Entry {
    /// Read the unquoted relative path without UTF-8 conversion.
    pub fn path(&self) -> &[u8] {
        match self {
            Self::Tracked { path, .. } | Self::Untracked { path } => path,
        }
    }
}

/// Extent of the reported path observations, independent of whether paths are dirty.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObservationCoverage {
    /// Git status coverage: tracked and non-ignored untracked paths; ignored bytes excluded.
    GitStatus,
    /// Required paths or properties were not completely observed.
    Incomplete,
}
/// Consistency observation, never an atomic snapshot claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObservationConsistency {
    /// The producer completed two matching bounded inspections.
    TwoMatchingSamples,
    /// Consistency could not be established.
    Indeterminate,
}

/// Validated state supplied through the infrastructure-to-core boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositoryState {
    repository: Option<RepositoryIdentity>,
    head: Head,
    entries: Vec<Entry>,
    coverage: ObservationCoverage,
    consistency: ObservationConsistency,
}
impl RepositoryState {
    /// Validate and sort entries. Missing facts stay explicit; they cannot bind successfully.
    pub fn new(
        repository: Option<RepositoryIdentity>,
        head: Head,
        mut entries: Vec<Entry>,
        coverage: ObservationCoverage,
        consistency: ObservationConsistency,
    ) -> Result<Self, BindingError> {
        if let Head::Attached { branch, .. } | Head::Unborn { branch } = &head {
            if branch.len() > MAX_OBSERVATION_BYTES {
                return Err(BindingError::ObservationLimit);
            }
            if branch.is_empty() || branch.iter().any(|b| b.is_ascii_control()) {
                return Err(BindingError::InvalidHead);
            }
        }
        if entries.len() > MAX_OBSERVATION_ENTRIES {
            return Err(BindingError::ObservationLimit);
        }
        let mut bytes = 0usize;
        for entry in &entries {
            let path = entry.path();
            bytes = bytes
                .checked_add(path.len())
                .ok_or(BindingError::ObservationLimit)?;
            if bytes > MAX_OBSERVATION_BYTES {
                return Err(BindingError::ObservationLimit);
            }
            if path.is_empty()
                || path.contains(&0)
                || path
                    .split(|b| *b == b'/')
                    .any(|c| c.is_empty() || c == b"." || c == b"..")
                || matches!(
                    entry,
                    Entry::Tracked {
                        staged: None,
                        unstaged: None,
                        ..
                    }
                )
            {
                return Err(BindingError::InvalidEntry);
            }
        }
        entries.sort_by(|a, b| a.path().cmp(b.path()));
        if entries
            .windows(2)
            .any(|pair| pair[0].path() == pair[1].path())
        {
            return Err(BindingError::DuplicateEntry);
        }
        Ok(Self {
            repository,
            head,
            entries,
            coverage,
            consistency,
        })
    }
    /// Read the reported repository identity, if established.
    pub fn repository(&self) -> Option<&RepositoryIdentity> {
        self.repository.as_ref()
    }
    /// Read the explicit HEAD state.
    pub fn head(&self) -> &Head {
        &self.head
    }
    /// Read sorted staged/unstaged/untracked path observations.
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }
    /// Read coverage without interpreting clean as full filesystem identity.
    pub fn coverage(&self) -> ObservationCoverage {
        self.coverage
    }
    /// Read the producer's bounded consistency observation.
    pub fn consistency(&self) -> ObservationConsistency {
        self.consistency
    }
}

/// Whether observed changes contradict the contract or need stronger identity evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorktreeRequirement {
    /// Any observed staged/unstaged/untracked change is incompatible.
    Clean,
    /// Changes are permitted in principle, but 002-A cannot identify their bytes;
    /// unchanged dirty status therefore remains insufficient for binding.
    AllowChanges,
}
/// Expected local repository, committed revision and working-state policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateRequirement {
    repository: RepositoryIdentity,
    commit: CandidateId,
    worktree: WorktreeRequirement,
}
impl CandidateRequirement {
    /// Declare requirements using already validated values. Commit syntax is not object proof.
    pub fn new(
        repository: RepositoryIdentity,
        commit: CandidateId,
        worktree: WorktreeRequirement,
    ) -> Self {
        Self {
            repository,
            commit,
            worktree,
        }
    }
    /// Read the expected repository/worktree identity.
    pub fn repository(&self) -> &RepositoryIdentity {
        &self.repository
    }
    /// Read the required committed revision.
    pub fn commit(&self) -> &CandidateId {
        &self.commit
    }
    /// Read the working-state requirement.
    pub fn worktree(&self) -> WorktreeRequirement {
        self.worktree
    }
}
/// A versioned binding contract, preserving the foundation task contract verbatim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateBindingContract {
    task: TaskContract,
    revision: ContractRevision,
    candidate: CandidateRequirement,
}
impl CandidateBindingContract {
    /// Associate a task revision with its candidate requirements; grants no authority.
    pub fn new(
        task: TaskContract,
        revision: ContractRevision,
        candidate: CandidateRequirement,
    ) -> Self {
        Self {
            task,
            revision,
            candidate,
        }
    }
    /// Read the complete foundation task and criterion.
    pub fn task(&self) -> &TaskContract {
        &self.task
    }
    /// Read the contract's explicit revision.
    pub fn revision(&self) -> ContractRevision {
        self.revision
    }
    /// Read the expected candidate properties.
    pub fn candidate(&self) -> &CandidateRequirement {
        &self.candidate
    }
}
/// Opaque, bounded comparison material supplied by the inspection mechanism.
///
/// This is equality material, not a snapshot hash. It can contain private data;
/// Debug deliberately omits it. Core never interprets its wire format.
#[derive(Clone, PartialEq, Eq)]
pub struct ComparisonBasis {
    profile: String,
    material: Vec<u8>,
}
impl ComparisonBasis {
    /// Bound profile text and at most three capture buffers plus length framing.
    pub fn new(profile: &str, material: Vec<u8>) -> Result<Self, BindingError> {
        if profile.len() > 4096 || material.len() > 3 * MAX_OBSERVATION_BYTES + 24 {
            return Err(BindingError::ObservationLimit);
        }
        if profile.trim().is_empty() || profile.chars().any(char::is_control) || material.is_empty()
        {
            return Err(BindingError::InvalidComparisonBasis);
        }
        Ok(Self {
            profile: profile.to_owned(),
            material,
        })
    }
    /// Read the comparison mechanism/profile identity, not its private material.
    pub fn profile(&self) -> &str {
        &self.profile
    }
    /// Explicitly borrow private comparison bytes for a qualified lossless adapter.
    ///
    /// These bytes may contain private repository context. Access grants no disclosure
    /// permission; callers must apply handling policy. Default Debug stays redacted.
    pub fn material(&self) -> &[u8] {
        &self.material
    }
}
impl fmt::Debug for ComparisonBasis {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ComparisonBasis")
            .field("profile", &self.profile)
            .field("material", &"<private comparison material>")
            .finish()
    }
}

/// One observation and the exact contract under which it was requested.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateObservation {
    id: ObservationId,
    contract: CandidateBindingContract,
    state: RepositoryState,
    basis: Option<ComparisonBasis>,
}
impl CandidateObservation {
    /// Retain immutable context. Inputs are producer assertions, not authenticated facts.
    pub fn new(
        id: ObservationId,
        contract: &CandidateBindingContract,
        state: RepositoryState,
        basis: Option<ComparisonBasis>,
    ) -> Self {
        Self {
            id,
            contract: contract.clone(),
            state,
            basis,
        }
    }
    /// Identify this observation within its producer's series.
    pub fn id(&self) -> &ObservationId {
        &self.id
    }
    /// Read the complete captured contract revision and requirements.
    pub fn contract(&self) -> &CandidateBindingContract {
        &self.contract
    }
    /// Read the captured repository state and limits.
    pub fn state(&self) -> &RepositoryState {
        &self.state
    }
    /// Read mechanism comparison material, if it was available.
    pub fn basis(&self) -> Option<&ComparisonBasis> {
        self.basis.as_ref()
    }
}

/// A demonstrated mismatch with a declared requirement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingMismatch {
    /// Observation was requested for another task.
    TaskIdentity,
    /// Repository/worktree identity differs.
    Repository,
    /// Committed revision differs.
    Commit,
    /// An observed unborn branch has no required commit.
    CommitAbsent,
    /// Changes violate a clean-worktree requirement.
    WorkingTree,
}
/// Why supplied observations cannot remain applicable to this binding request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingStaleness {
    /// Observation belongs to a different revision of the task's contract.
    ContractRevision,
    /// Reobservation reuses an ID or predates the baseline in the same series.
    ObservationOrder,
    /// Relevant typed repository state differs between captures.
    ObservedStateChanged,
}
/// Missing information, distinct from a demonstrated requirement violation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingInsufficiency {
    /// No baseline observation was supplied.
    MissingObservation,
    /// A separate reobservation was not supplied.
    ReobservationRequired,
    /// Observation ordering cannot be compared across producer scopes.
    DifferentObservationScope,
    /// Required path coverage was incomplete.
    IncompleteCoverage,
    /// Bounded inspection consistency was not established.
    IndeterminateConsistency,
    /// Repository identity was not established.
    MissingRepository,
    /// HEAD identity was not established.
    MissingCommit,
    /// Dirty path/status records do not identify uncommitted contents.
    UncommittedIdentity,
    /// Mechanism/configuration/index comparison material was not supplied.
    MissingComparisonBasis,
}
/// Immutable association supported only within the captured Git status coverage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundCandidate {
    baseline: ObservationId,
    observation: CandidateObservation,
}
impl BoundCandidate {
    /// Identify the baseline to which the later observation was compared.
    pub fn baseline(&self) -> &ObservationId {
        &self.baseline
    }
    /// Retain the task revision, later observation and its coverage/consistency limitations.
    pub fn observation(&self) -> &CandidateObservation {
        &self.observation
    }
}
/// Binding-domain correspondence; these are not evidence verdicts or authorization states.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BindingDecision {
    /// Requirements match within the supplied observations' Git status coverage only.
    Compatible(Box<BoundCandidate>),
    /// A supplied fact contradicts a requirement.
    Incompatible(BindingMismatch),
    /// A revision, ordering or observed-state change invalidates this comparison.
    Stale(BindingStaleness),
    /// Available information cannot support the requested association.
    Insufficient(BindingInsufficiency),
}

fn applicability(
    contract: &CandidateBindingContract,
    observed: &CandidateObservation,
) -> Result<Option<BindingDecision>, BindingError> {
    if contract.task.id() != observed.contract.task.id() {
        return Ok(Some(BindingDecision::Incompatible(
            BindingMismatch::TaskIdentity,
        )));
    }
    if contract.revision != observed.contract.revision {
        return Ok(Some(BindingDecision::Stale(
            BindingStaleness::ContractRevision,
        )));
    }
    if contract != &observed.contract {
        return Err(BindingError::ContractRevisionReused);
    }
    Ok(None)
}

fn correspondence(
    contract: &CandidateBindingContract,
    state: &RepositoryState,
) -> Option<BindingDecision> {
    let Some(repository) = &state.repository else {
        return Some(BindingDecision::Insufficient(
            BindingInsufficiency::MissingRepository,
        ));
    };
    if repository != &contract.candidate.repository {
        return Some(BindingDecision::Incompatible(BindingMismatch::Repository));
    }
    match &state.head {
        Head::Attached { commit, .. } | Head::Detached { commit }
            if commit != &contract.candidate.commit =>
        {
            Some(BindingDecision::Incompatible(BindingMismatch::Commit))
        }
        Head::Unborn { .. } => Some(BindingDecision::Incompatible(BindingMismatch::CommitAbsent)),
        Head::Unknown => Some(BindingDecision::Insufficient(
            BindingInsufficiency::MissingCommit,
        )),
        _ => None,
    }
}

/// Compare a baseline with a separately captured, later observation of this exact contract.
///
/// The caller/producer must actually reinspect; increasing a sequence does not do
/// that. No wall clock, authenticated freshness or permanently current result is
/// implied. Both observations must have the same scope and increasing sequence.
/// Deterministic priority: context, ordering, candidate facts (later then baseline),
/// coverage/consistency, changed state, dirty policy, scoped compatibility.
/// Missing/uncertain information is never promoted to compatibility.
pub fn bind_candidate(
    contract: &CandidateBindingContract,
    baseline: Option<&CandidateObservation>,
    reobserved: Option<&CandidateObservation>,
) -> Result<BindingDecision, BindingError> {
    let Some(baseline) = baseline else {
        return Ok(BindingDecision::Insufficient(
            BindingInsufficiency::MissingObservation,
        ));
    };
    if let Some(result) = applicability(contract, baseline)? {
        return Ok(result);
    }
    let Some(latest) = reobserved else {
        return Ok(BindingDecision::Insufficient(
            BindingInsufficiency::ReobservationRequired,
        ));
    };
    if let Some(result) = applicability(contract, latest)? {
        return Ok(result);
    }
    if baseline.id.scope != latest.id.scope {
        return Ok(BindingDecision::Insufficient(
            BindingInsufficiency::DifferentObservationScope,
        ));
    }
    if latest.id.sequence <= baseline.id.sequence {
        return Ok(BindingDecision::Stale(BindingStaleness::ObservationOrder));
    }
    for state in [&latest.state, &baseline.state] {
        if let Some(result) = correspondence(contract, state) {
            return Ok(result);
        }
    }
    for state in [&latest.state, &baseline.state] {
        if state.coverage != ObservationCoverage::GitStatus {
            return Ok(BindingDecision::Insufficient(
                BindingInsufficiency::IncompleteCoverage,
            ));
        }
        if state.consistency != ObservationConsistency::TwoMatchingSamples {
            return Ok(BindingDecision::Insufficient(
                BindingInsufficiency::IndeterminateConsistency,
            ));
        }
    }
    if baseline.basis.is_none() || latest.basis.is_none() {
        return Ok(BindingDecision::Insufficient(
            BindingInsufficiency::MissingComparisonBasis,
        ));
    }
    if baseline.state != latest.state || baseline.basis != latest.basis {
        return Ok(BindingDecision::Stale(
            BindingStaleness::ObservedStateChanged,
        ));
    }
    if !latest.state.entries.is_empty() {
        return Ok(match contract.candidate.worktree {
            WorktreeRequirement::Clean => {
                BindingDecision::Incompatible(BindingMismatch::WorkingTree)
            }
            WorktreeRequirement::AllowChanges => {
                BindingDecision::Insufficient(BindingInsufficiency::UncommittedIdentity)
            }
        });
    }
    Ok(BindingDecision::Compatible(Box::new(BoundCandidate {
        baseline: baseline.id.clone(),
        observation: latest.clone(),
    })))
}
