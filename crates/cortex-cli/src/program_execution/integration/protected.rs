//! Actual bounded Git observations for the selected protected material profile.

use super::*;
use crate::inspect_candidate;
use cortex_core::parallel::{NotificationId, PredicateId};
use cortex_evidence::{
    ExecutionObservation,
    program::material::{
        MaterialFile, MaterialMapping, MaterialMode, MaterialObservation, MaterialSubject,
    },
    readiness::{EdgeEvidenceContract, EdgeScopeSnapshot},
    unified::{OracleConclusion, OracleEvaluation, VerificationOutcome},
};
use cortex_harness::artifact::artifact_digest;

/// Request to observe the exact bytes and mode offered for one prospective edge release.
pub struct MaterialReleaseRequest<'a> {
    /// Exact accepted edge, criterion correspondence and artifact candidate.
    pub contract: &'a EdgeEvidenceContract,
    /// One of the edge's explicit verification predicates.
    pub predicate: &'a PredicateId,
    /// Exact proposed release operation.
    pub release: OperationId,
    /// Exact proposed coordinator review.
    pub review: NotificationId,
    /// Retained producer artifact report.
    pub artifact_report: NotificationId,
    /// Literal committed file in the producer artifact candidate.
    pub source: &'a SourcePath,
    /// Literal destination to be covered by this edge in the integration task.
    pub destination: &'a SourcePath,
    /// Expected literal producer bytes selected independently under the criterion.
    pub expected: &'a [u8],
    /// Expected regular Git mode.
    pub expected_mode: MaterialMode,
    /// Complete bound for expected and observed bytes.
    pub max_bytes: usize,
    /// Exact journal predecessor for the new observation.
    pub expected_revision: u64,
    /// Fresh Git candidate capture identity.
    pub candidate_observation: ObservationId,
}

/// Request to observe an integrated candidate against accepted material and a separate criterion.
pub struct ProtectedIntegrationRequest<'a> {
    /// Exact two-constituent task, file mapping and independent output oracle.
    pub integration: IntegrationRequest<'a>,
    /// Exact verification profile to be examined and assessed under review.
    pub profile: VerificationProfile,
    /// Expected regular Git mode for the independent integration output.
    pub expected_output_mode: MaterialMode,
}

/// Retained observation input, actual oracle result and original store receipt.
pub struct AcceptedMaterialObservation {
    /// Exact operation for read-only reconciliation or inspection.
    pub operation: Operation,
    /// Original admission receipt. This supplies no assessment.
    pub receipt: AdmissionReceipt,
    /// Actual comparison result. Consequential decisions require separate assessment and review.
    pub satisfied: bool,
}

/// Observe exact release material against an independent literal byte and mode oracle.
///
/// The scoped review grant is checked by canonical admission. The observation
/// remains separate from the terminal assessment and the prospective release.
pub fn observe_release_material(
    store: &ProgramStore,
    key: &StoreKey,
    request: MaterialReleaseRequest<'_>,
    observation: IntegrationObservationRequest,
) -> Result<AcceptedMaterialObservation, IntegrationError> {
    let kernel = current_material_kernel(store, key, request.expected_revision)?;
    let state = kernel.state();
    if &EdgeScopeSnapshot::capture(state.readiness(), request.contract.scope().edge())
        .map_err(|_| IntegrationError::Context)?
        != request.contract.scope()
    {
        return Err(IntegrationError::Context);
    }
    let predicate = request
        .contract
        .correspondences()
        .iter()
        .find(|p| p.predicate() == request.predicate)
        .ok_or(IntegrationError::Context)?;
    let candidate = request.contract.artifact_requirement();
    let mut remaining = material_budget(&kernel, request.max_bytes, request.expected.len())?;
    let file = read_material(candidate, request.source, &mut remaining)?;
    let material = material_file(request.source, &file)?;
    let satisfied = file.bytes == request.expected && material.mode() == request.expected_mode;
    let capture = inspect_candidate(
        candidate.repository().root(),
        predicate.verification(),
        request.candidate_observation,
    )
    .map_err(IntegrationError::Blob)?;
    drop(kernel);
    let report = material_report(
        observation.observation,
        capture,
        predicate.profile().clone(),
        satisfied,
    )?;
    let operation = Operation {
        id: observation.operation,
        expected: request.expected_revision,
        grant: observation.review,
        command: Command::ObserveMaterial(Box::new(MaterialObservation {
            report,
            subject: MaterialSubject::Release {
                contract: Box::new(request.contract.clone()),
                release: request.release,
                review: request.review,
                artifact_report: request.artifact_report,
                mapping: MaterialMapping {
                    source: material,
                    destination: request.destination.clone(),
                },
            },
        })),
    };
    admit_material_observation(store, key, operation, satisfied)
}

/// Observe an integrated candidate only using current accepted typed constituent material.
///
/// This path reads the exact immutable producer and destination files specified by
/// the retained material releases, compares actual bytes and modes, and performs
/// a separate output oracle. The Store revalidates the exact predecessor,
/// releases and authority at admission. A later supported assessment and
/// explicit integration decision are separate operations.
pub fn observe_protected_integration(
    store: &ProgramStore,
    key: &StoreKey,
    request: ProtectedIntegrationRequest<'_>,
    observation: IntegrationObservationRequest,
) -> Result<AcceptedMaterialObservation, IntegrationError> {
    let selection = request.integration;
    let kernel = current_material_kernel(store, key, selection.expected_revision)?;
    let scope = kernel
        .state()
        .readiness()
        .scope(selection.task)
        .map_err(|_| IntegrationError::Context)?;
    let candidate = scope.context().contract().candidate();
    let mut remaining = material_budget(
        &kernel,
        selection.max_bytes,
        selection.oracle.expected.len(),
    )?;
    let [left, right] = &selection.constituents;
    if left.release == right.release
        || left.integrated_path == right.integrated_path
        || selection.oracle.path == left.integrated_path
        || selection.oracle.path == right.integrated_path
        || selection.oracle.expected.is_empty()
        || candidate.worktree() != WorktreeRequirement::Clean
    {
        return Err(IntegrationError::Context);
    }
    for selected in &selection.constituents {
        let (release, mapping) = kernel
            .accepted_material(selected.release)
            .map_err(|_| IntegrationError::Release)?;
        let worker = release.contract.artifact_requirement();
        if release.contract.scope().consumer() != &scope
            || worker.commit() == candidate.commit()
            || mapping.source.path() != selected.worker_path
            || &mapping.destination != selected.integrated_path
        {
            return Err(IntegrationError::Release);
        }
        let before = read_material(worker, mapping.source.path(), &mut remaining)?;
        let after = read_material(candidate, &mapping.destination, &mut remaining)?;
        if mapping.source != material_file(mapping.source.path(), &before)? || before != after {
            return Err(IntegrationError::Release);
        }
    }
    let output = read_material(candidate, selection.oracle.path, &mut remaining)?;
    let material = material_file(selection.oracle.path, &output)?;
    let satisfied = output.bytes == selection.oracle.expected
        && material.mode() == request.expected_output_mode;
    let captured = check_workspace(scope.context(), selection.workspace_observation)
        .map_err(IntegrationError::Workspace)?;
    let report = material_report(
        observation.observation,
        captured.observation().clone(),
        request.profile,
        satisfied,
    )?;
    let mut releases = [left.release.clone(), right.release.clone()];
    releases.sort_by(|a, b| a.as_str().cmp(b.as_str()));
    let operation = Operation {
        id: observation.operation,
        expected: selection.expected_revision,
        grant: observation.review,
        command: Command::ObserveMaterial(Box::new(MaterialObservation {
            report,
            subject: MaterialSubject::Integration {
                scope: Box::new(scope),
                releases,
                output: material,
            },
        })),
    };
    drop(kernel);
    admit_material_observation(store, key, operation, satisfied)
}

fn current_material_kernel(
    store: &ProgramStore,
    key: &StoreKey,
    expected: u64,
) -> Result<CoordinationKernel, IntegrationError> {
    if store.profile() != ProgramProfile::ProtectedMaterialV5 {
        return Err(IntegrationError::Context);
    }
    let kernel = store
        .load(key, Access::ReadOnly)
        .map_err(IntegrationError::Store)?;
    if kernel.revision() != expected {
        return Err(IntegrationError::Context);
    }
    Ok(kernel)
}

fn material_budget(
    kernel: &CoordinationKernel,
    max: usize,
    expected: usize,
) -> Result<usize, IntegrationError> {
    if max == 0 || max > kernel.state().definition().limits().bytes {
        return Err(IntegrationError::Limit);
    }
    max.checked_sub(expected).ok_or(IntegrationError::Limit)
}

fn material_file(
    path: &SourcePath,
    file: &CommittedFile,
) -> Result<MaterialFile, IntegrationError> {
    MaterialFile::new(
        path.clone(),
        if file.executable {
            MaterialMode::Executable
        } else {
            MaterialMode::Regular
        },
        u64::try_from(file.bytes.len()).map_err(|_| IntegrationError::Limit)?,
        artifact_digest(&file.bytes),
    )
    .map_err(|_| IntegrationError::Context)
}

fn material_report(
    id: ObservationId,
    capture: cortex_core::binding::CandidateObservation,
    profile: VerificationProfile,
    satisfied: bool,
) -> Result<ReportedObservation, IntegrationError> {
    let now = SystemTime::now();
    let times = ObservationTimes::new(now, now).map_err(IntegrationError::Evidence)?;
    let oracle = OracleEvaluation::new(
        if satisfied {
            OracleConclusion::Satisfied
        } else {
            OracleConclusion::Violated
        },
        "Compare the immutable file bytes and Git mode with the independent exact criterion",
    )
    .map_err(IntegrationError::Evidence)?;
    let outcome =
        VerificationOutcome::new(ExecutionObservation::Exited { code: 0 }, 1, Some(oracle))
            .map_err(IntegrationError::Evidence)?;
    ReportedObservation::new(
        id,
        capture,
        profile,
        times,
        ObservationKind::Execution(outcome),
        "Bounded Git object queries and exact material comparison completed",
    )
    .map_err(IntegrationError::Evidence)
}

fn admit_material_observation(
    store: &ProgramStore,
    key: &StoreKey,
    operation: Operation,
    satisfied: bool,
) -> Result<AcceptedMaterialObservation, IntegrationError> {
    let receipt =
        store
            .admit(key, operation.clone())
            .map_err(|failure| IntegrationError::Admission {
                operation: Box::new(operation.clone()),
                failure,
            })?;
    Ok(AcceptedMaterialObservation {
        operation,
        receipt,
        satisfied,
    })
}
