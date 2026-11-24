//! Bound examinations of committed artifacts in a distinct integration candidate.

use super::{ExecutionError, check_workspace};
use crate::{InspectionError, inspection::committed_blob_bounded};
use cortex_core::{
    TaskId,
    binding::{BoundCandidate, ObservationId, WorktreeRequirement},
    knowledge::SourcePath,
    parallel::EdgeDecision,
    program::OperationId,
};
use cortex_evidence::program::{Command, CoordinationKernel, ReleaseInput};
use cortex_storage::{
    history_store::{Access, StoreKey},
    program_store::{ProgramProfile, ProgramStore, ProgramStoreError},
};

/// One exact durable producer release and its scoped committed file mapping.
pub struct IntegrationConstituent<'a> {
    /// Current retained release operation contributing to the integration task.
    pub release: &'a OperationId,
    /// Regular committed file in the artifact candidate from Evidence.
    pub worker_path: &'a SourcePath,
    /// Corresponding separate regular committed file in the integration candidate.
    pub integrated_path: &'a SourcePath,
}

/// Explicit exact oracle over a separate committed integration output.
pub struct IntegrationOracle<'a> {
    /// Separate regular committed file containing the integrated result.
    pub path: &'a SourcePath,
    /// Independently chosen expected bytes under the accepted task criterion.
    pub expected: &'a [u8],
}

/// Bounded examination of the two-constituent integration profile.
pub struct IntegrationRequest<'a> {
    /// Current accepted integration task, with its own distinct candidate.
    pub task: &'a TaskId,
    /// Expected canonical revision at which both releases are current.
    pub expected_revision: u64,
    /// The two independently accepted artifact releases and their file mappings.
    pub constituents: [IntegrationConstituent<'a>; 2],
    /// Independently selected expected combined output.
    pub oracle: IntegrationOracle<'a>,
    /// Total bound for the five committed blob reads and expected oracle bytes.
    pub max_bytes: usize,
    /// Fresh bounded Git observation of the integrated candidate.
    pub workspace_observation: ObservationId,
}

/// Why the integrated candidate could not be examined in the selected scope.
#[derive(Debug)]
pub enum IntegrationError {
    /// The canonical supervised program could not be observed.
    Store(ProgramStoreError),
    /// Different or changed revision, incomplete release or wrong task/candidate.
    Context,
    /// A required constituent is no longer the current supported release.
    Release,
    /// Invalid, empty, or exceeded material bound.
    Limit,
    /// Bounded lookup of a declared regular committed file failed.
    Blob(InspectionError),
    /// Actual integration workspace did not correspond to the accepted baseline.
    Workspace(ExecutionError),
}
impl std::fmt::Display for IntegrationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Store(_) => "integration examination: canonical state unavailable",
            Self::Context => "integration examination: current scope unavailable",
            Self::Release => "integration examination: current artifact release unavailable",
            Self::Limit => "integration examination: material bound exceeded",
            Self::Blob(_) => "integration examination: committed file unavailable",
            Self::Workspace(_) => "integration examination: candidate workspace unavailable",
        })
    }
}
impl std::error::Error for IntegrationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Store(error) => Some(error),
            Self::Blob(error) => Some(error),
            Self::Workspace(error) => Some(error),
            _ => None,
        }
    }
}

/// Independent, actual byte check results, without authority or promotion.
pub struct IntegrationInspection {
    /// Exact reconstructed journal revision of the scoped comparison.
    pub revision: u64,
    /// Current integration candidate and its observed repository coverage.
    pub candidate: Box<BoundCandidate>,
    /// Whether each contributor's committed file is present byte-for-byte.
    pub constituents_match: [bool; 2],
    /// Whether the separate integrated output matches the supplied exact oracle.
    pub output_matches: bool,
}

/// Compare actual committed worker and integration bytes in a canonical context.
///
/// The integration task must have exactly these two current incoming releases.
/// The source paths and output oracle are explicit selections within the
/// accepted local scope. The two constituents are pinned by their retained
/// Evidence artifact candidates. Git ref labels and ignored worktree bytes
/// cannot supply either material object. Results are read-only observations
/// for the separately authorized assessment owner.
pub fn inspect_integrated_candidate(
    store: &ProgramStore,
    key: &StoreKey,
    request: IntegrationRequest<'_>,
) -> Result<IntegrationInspection, IntegrationError> {
    if store.profile() != ProgramProfile::SupervisedFixturesV3 {
        return Err(IntegrationError::Context);
    }
    let kernel = store
        .load(key, Access::ReadOnly)
        .map_err(IntegrationError::Store)?;
    let state = kernel.state();
    let scope = state
        .readiness()
        .scope(request.task)
        .map_err(|_| IntegrationError::Context)?;
    let integrated = scope.context().contract().candidate();
    let [left, right] = &request.constituents;
    if kernel.revision() != request.expected_revision
        || integrated.worktree() != WorktreeRequirement::Clean
        || state
            .definition()
            .graph()
            .edges()
            .iter()
            .filter(|edge| edge.consumer() == request.task)
            .count()
            != 2
        || left.integrated_path == right.integrated_path
        || request.oracle.path == left.integrated_path
        || request.oracle.path == right.integrated_path
    {
        return Err(IntegrationError::Context);
    }
    if request.oracle.expected.is_empty()
        || request.max_bytes == 0
        || request.max_bytes > state.definition().limits().bytes
    {
        return Err(IntegrationError::Limit);
    }
    let mut remaining = request.max_bytes;
    use_bytes(&mut remaining, request.oracle.expected.len())?;

    let left_release = retained_release(&kernel, left.release, request.task)?;
    let right_release = retained_release(&kernel, right.release, request.task)?;
    if left_release.contract.scope().edge() == right_release.contract.scope().edge()
        || left_release.contract.scope().producer().task()
            == right_release.contract.scope().producer().task()
    {
        return Err(IntegrationError::Context);
    }

    let mut constituents_match = [false; 2];
    for (i, (selected, release)) in request
        .constituents
        .iter()
        .zip([left_release, right_release])
        .enumerate()
    {
        let worker = release.contract.artifact_requirement();
        if integrated.commit() == worker.commit() {
            return Err(IntegrationError::Context);
        }
        let before = committed_blob_bounded(
            worker.repository(),
            worker.commit(),
            selected.worker_path,
            remaining,
        )
        .map_err(IntegrationError::Blob)?;
        use_bytes(&mut remaining, before.len())?;
        let after = committed_blob_bounded(
            integrated.repository(),
            integrated.commit(),
            selected.integrated_path,
            remaining,
        )
        .map_err(IntegrationError::Blob)?;
        use_bytes(&mut remaining, after.len())?;
        constituents_match[i] = before == after;
    }
    let output = committed_blob_bounded(
        integrated.repository(),
        integrated.commit(),
        request.oracle.path,
        remaining,
    )
    .map_err(IntegrationError::Blob)?;
    use_bytes(&mut remaining, output.len())?;
    let output_matches = output == request.oracle.expected;
    let candidate = check_workspace(scope.context(), request.workspace_observation)
        .map_err(IntegrationError::Workspace)?;
    drop(kernel);
    if store
        .load(key, Access::ReadOnly)
        .map_err(IntegrationError::Store)?
        .revision()
        != request.expected_revision
    {
        return Err(IntegrationError::Context);
    }
    Ok(IntegrationInspection {
        revision: request.expected_revision,
        candidate,
        constituents_match,
        output_matches,
    })
}

fn use_bytes(remaining: &mut usize, bytes: usize) -> Result<(), IntegrationError> {
    *remaining = remaining
        .checked_sub(bytes)
        .ok_or(IntegrationError::Limit)?;
    Ok(())
}

fn retained_release<'a>(
    kernel: &'a CoordinationKernel,
    operation: &OperationId,
    task: &TaskId,
) -> Result<&'a ReleaseInput, IntegrationError> {
    let retained = kernel
        .operations()
        .iter()
        .find(|retained| &retained.id == operation);
    let Some(Command::Release(release)) = retained.map(|retained| &retained.command) else {
        return Err(IntegrationError::Release);
    };
    let scope = release.contract.scope();
    let readiness = kernel.state().readiness();
    let state = readiness
        .edge(scope.edge())
        .map_err(|_| IntegrationError::Release)?;
    let producer = readiness
        .scope(scope.producer().task())
        .map_err(|_| IntegrationError::Release)?;
    let consumer = readiness
        .scope(task)
        .map_err(|_| IntegrationError::Release)?;
    if scope.consumer().task() != task
        || &producer != scope.producer()
        || &consumer != scope.consumer()
        || !kernel
            .decisions()
            .iter()
            .rev()
            .find(|decision| &decision.edge == scope.edge() && decision.released)
            .is_some_and(|decision| &decision.operation == operation)
        || !matches!(state.decision(), EdgeDecision::Released { review, .. } if review == release.review.id())
    {
        return Err(IntegrationError::Release);
    }
    Ok(release)
}
