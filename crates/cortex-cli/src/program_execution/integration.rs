//! Bound examinations of selected committed files in a distinct integration candidate.

use super::{ExecutionError, check_workspace};
use crate::{InspectionError, inspection::committed_file_bounded};
use cortex_core::{
    TaskId,
    binding::{BoundCandidate, ObservationId, WorktreeRequirement},
    knowledge::SourcePath,
    parallel::EdgeDecision,
    program::{CoordinationGrant, CoordinationPermission, OperationId},
};
use cortex_evidence::{
    Provenance,
    program::{Command, CoordinationKernel, Operation, ReleaseInput},
    unified::{
        ObservationKind, ObservationTimes, ReportedObservation, SemanticError, VerificationProfile,
    },
};
use cortex_harness::artifact::artifact_sha256;
use cortex_storage::{
    history_store::{Access, StoreKey},
    program_store::{
        AdmissionFailure, AdmissionReceipt, ProgramProfile, ProgramStore, ProgramStoreError,
    },
};

use std::time::SystemTime;

/// One exact durable producer release and its scoped committed file mapping.
pub struct IntegrationConstituent<'a> {
    /// Current retained release operation contributing to the integration task.
    pub release: &'a OperationId,
    /// Explicitly selected regular file within the release's artifact candidate.
    /// The existing release does not attest that this path is the accepted artifact.
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
    /// The explicit metadata grant does not cover review of the integration task.
    Authority,
    /// Existing Evidence could not construct the scoped inference.
    Evidence(SemanticError),
    /// The exact input and uncertainty remain available for canonical reconciliation.
    Admission {
        /// Original operation and exact reported content.
        operation: Box<Operation>,
        /// Original canonical refusal or uncertainty.
        failure: AdmissionFailure,
    },
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
            Self::Authority => "integration examination: scoped review unavailable",
            Self::Evidence(_) => "integration examination: inference unavailable",
            Self::Admission { .. } => "integration examination: observation admission unavailable",
        })
    }
}
impl std::fmt::Debug for IntegrationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self, f)
    }
}
impl std::error::Error for IntegrationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Store(error) => Some(error),
            Self::Blob(error) => Some(error),
            Self::Workspace(error) => Some(error),
            Self::Evidence(error) => Some(error),
            Self::Admission { failure, .. } => Some(failure),
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
    /// Whether each selected committed file pair has identical bytes and Git file mode.
    pub constituents_match: [bool; 2],
    /// Whether the separate integrated output matches the supplied exact oracle.
    pub output_matches: bool,
    /// Actual integrated output digest independently reconstructed from the commit.
    pub output_sha256: String,
}

/// Compare actual committed worker and integration files in a canonical context.
///
/// The integration task must have exactly these two current incoming releases.
/// Select separate committed file objects by commit and path. Each file is read
/// within the recorded repository and compared by bytes and Git mode. The
/// releases identify the artifact candidates. Path selection and the output
/// oracle are supplied separately.
/// Results are read-only observations for the separately authorized assessment owner.
pub fn inspect_integrated_candidate(
    store: &ProgramStore,
    key: &StoreKey,
    request: IntegrationRequest<'_>,
) -> Result<IntegrationInspection, IntegrationError> {
    inspect_integrated_candidate_inner(store, key, &request, None)
}

fn inspect_integrated_candidate_inner(
    store: &ProgramStore,
    key: &StoreKey,
    request: &IntegrationRequest<'_>,
    review: Option<&CoordinationGrant>,
) -> Result<IntegrationInspection, IntegrationError> {
    if store.profile() != ProgramProfile::SupervisedFixturesV3 {
        return Err(IntegrationError::Context);
    }
    let kernel = store
        .load(key, Access::ReadOnly)
        .map_err(IntegrationError::Store)?;
    let state = kernel.state();
    if let Some(review) = review {
        review
            .check(
                state.definition(),
                CoordinationPermission::Review,
                std::slice::from_ref(request.task),
            )
            .map_err(|_| IntegrationError::Authority)?;
    }
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
        || (left_release.contract.artifact_requirement().commit()
            == right_release.contract.artifact_requirement().commit()
            && left.worker_path == right.worker_path)
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
        let before = committed_file_bounded(
            worker.repository(),
            worker.commit(),
            selected.worker_path,
            remaining,
        )
        .map_err(IntegrationError::Blob)?;
        use_bytes(&mut remaining, before.bytes.len())?;
        let after = committed_file_bounded(
            integrated.repository(),
            integrated.commit(),
            selected.integrated_path,
            remaining,
        )
        .map_err(IntegrationError::Blob)?;
        use_bytes(&mut remaining, after.bytes.len())?;
        constituents_match[i] = before == after;
    }
    let output = committed_file_bounded(
        integrated.repository(),
        integrated.commit(),
        request.oracle.path,
        remaining,
    )
    .map_err(IntegrationError::Blob)?;
    use_bytes(&mut remaining, output.bytes.len())?;
    let output_matches = output.bytes == request.oracle.expected;
    let candidate = check_workspace(scope.context(), request.workspace_observation.clone())
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
        output_sha256: artifact_sha256(&output.bytes),
    })
}

/// Explicit metadata authority and identities for one integration observation.
pub struct IntegrationObservationRequest {
    /// Fresh, exact ProgramStore operation identity.
    pub operation: OperationId,
    /// Scoped review permission from the selected current definition.
    pub review: CoordinationGrant,
    /// Observation identity distinct from the Git candidate capture.
    pub observation: ObservationId,
}

/// Readback plus the original durable observation input and receipt.
pub struct IntegrationObservation {
    /// Actual constituent and integrated result observations.
    pub inspection: IntegrationInspection,
    /// Exact admitted operation. It supplies no subsequent authority.
    pub operation: Operation,
    /// Canonical metadata admission receipt.
    pub receipt: AdmissionReceipt,
}

/// Collect and retain one candidate-scoped comparison through existing Evidence.
///
/// The observer records an inference from actual committed-material comparison.
/// Only a separately authorized assessment can provide a judgment. New
/// observations remain bound to the examined revision, and an uncertain
/// admit returns the original operation for canonical reconciliation.
pub fn observe_integrated_candidate(
    store: &ProgramStore,
    key: &StoreKey,
    request: IntegrationRequest<'_>,
    observation: IntegrationObservationRequest,
) -> Result<IntegrationObservation, IntegrationError> {
    let inspection =
        inspect_integrated_candidate_inner(store, key, &request, Some(&observation.review))?;
    let [left, right] = &request.constituents;
    let inputs = format!(
        "left-release={} worker-path={} integrated-path={}; right-release={} worker-path={} integrated-path={}",
        left.release.as_str(),
        artifact_sha256(left.worker_path.bytes()),
        artifact_sha256(left.integrated_path.bytes()),
        right.release.as_str(),
        artifact_sha256(right.worker_path.bytes()),
        artifact_sha256(right.integrated_path.bytes()),
    );
    let configuration = format!(
        "output-path={} expected-output={} max-bytes={}",
        artifact_sha256(request.oracle.path.bytes()),
        artifact_sha256(request.oracle.expected),
        request.max_bytes,
    );
    let provenance = Provenance::new(
        "Cortex committed integration comparison",
        &inputs,
        &configuration,
    )
    .map_err(SemanticError::from)
    .map_err(IntegrationError::Evidence)?;
    let profile = VerificationProfile::new(
        provenance,
        "2",
        "cooperative local Linux source fixture",
        &[],
    )
    .map_err(IntegrationError::Evidence)?;
    let now = SystemTime::now();
    let observed = ReportedObservation::new(
        observation.observation,
        inspection.candidate.observation().clone(),
        profile,
        ObservationTimes::new(now, now).map_err(IntegrationError::Evidence)?,
        ObservationKind::Inference,
        &format!(
            "selected-left-bytes-and-mode-equal={} selected-right-bytes-and-mode-equal={} output-matches-oracle={} observed-output={}",
            inspection.constituents_match[0],
            inspection.constituents_match[1],
            inspection.output_matches,
            inspection.output_sha256,
        ),
    )
    .map_err(IntegrationError::Evidence)?;
    let operation = Operation {
        id: observation.operation,
        expected: request.expected_revision,
        grant: observation.review,
        command: Command::Observe(observed),
    };
    match store.admit(key, operation.clone()) {
        Ok(receipt) => Ok(IntegrationObservation {
            inspection,
            operation,
            receipt,
        }),
        Err(failure) => Err(IntegrationError::Admission {
            operation: Box::new(operation),
            failure,
        }),
    }
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
