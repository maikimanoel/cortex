//! Provider-free composition of canonical executor input, local workspace
//! correspondence, the trusted fixture supervisor and artifact capture.
//!
//! The supported profile is cooperative development in an unchanged owned
//! workspace. This is not a new containment backend or an acceptance authority.

mod codex_cli;
mod integration;
mod workspace;
pub use codex_cli::{CodexCliHold, CodexCliInspection, inspect_codex_cli_invocation};
pub use integration::{
    IntegrationConstituent, IntegrationError, IntegrationInspection, IntegrationOracle,
    IntegrationRequest, inspect_integrated_candidate,
};
pub use workspace::{
    FixtureWorkspace, FixtureWorkspaceRequest, WorkspaceError, WorkspaceFixtureRun,
    prepare_fixture_workspace, run_workspace_fixture,
};

use crate::{InspectionError, inspect_candidate, program_preparation::ExecutorPreparation};
use cortex_core::{
    binding::{
        BindingDecision, BindingError, BoundCandidate, ObservationId, RepositoryIdentity,
        bind_candidate,
    },
    parallel::{ArtifactReference, TaskContext},
    program::{
        executor::{ExecutorError, ObservationKind},
        operational::{EffectEvent, EffectId, EffectScope},
    },
};
use cortex_evidence::program::{Command, Operation};
use cortex_harness::{
    CaptureIds, HarnessError, PinnedFixture, RunResult,
    artifact::{ArtifactError, CapturedArtifact, capture_artifact},
    execute_fixture,
};
use cortex_storage::{
    history_store::{Access, StoreKey},
    program_store::{ProgramProfile, ProgramStore, ProgramStoreError},
};
use std::{error::Error, fmt, path::Path, sync::atomic::AtomicBool};

/// Refusal from the selected composition or an existing owner.
#[derive(Debug)]
pub enum ExecutionError {
    /// Only the trusted direct-child fixture profile is supported.
    UnsupportedProfile,
    /// The dispatch does not match the exact current retained predecessor.
    Dispatch,
    /// The original canonical preparation or its exact input is unavailable.
    PreparedInput,
    /// Core rejected the exact retained input in the current state.
    Input(ExecutorError),
    /// A canonical readout failed.
    Store(ProgramStoreError),
    /// A separate bounded Git observation failed.
    Inspection(InspectionError),
    /// The Core observation context is inconsistent.
    Binding(BindingError),
    /// Core could not bind the current workspace to the retained contract.
    Workspace(BindingDecision),
    /// The trusted fixture supervisor refused the request.
    Harness(HarnessError),
    /// The canonical process or protected file capture refused the output.
    Artifact(ArtifactError),
    /// Observed bytes contradict the independently selected expected digest.
    ArtifactDigest,
    /// Canonical state changed during artifact capture.
    ChangedRevision,
    /// Fresh attempt output custody could not be established.
    AttemptWorkspace(WorkspaceError),
}

impl fmt::Display for ExecutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::UnsupportedProfile => "program execution: unsupported fixture profile",
            Self::Dispatch => "program execution: current dispatch unavailable",
            Self::PreparedInput => "program execution: exact prepared input unavailable",
            Self::Input(_) => "program execution: retained input refused by Core",
            Self::Store(_) => "program execution: canonical state unavailable",
            Self::Inspection(_) => "program execution: workspace inspection failed",
            Self::Binding(_) | Self::Workspace(_) => {
                "program execution: current workspace correspondence unavailable"
            }
            Self::Harness(_) => "program execution: supervisor refused dispatch",
            Self::Artifact(_) => "program execution: artifact capture refused",
            Self::ArtifactDigest => "program execution: artifact digest mismatch",
            Self::ChangedRevision => "program execution: canonical revision changed during capture",
            Self::AttemptWorkspace(error) => return fmt::Display::fmt(error, f),
        })
    }
}

impl Error for ExecutionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Input(error) => Some(error),
            Self::Store(error) => Some(error),
            Self::Inspection(error) => Some(error),
            Self::Binding(error) => Some(error),
            Self::Harness(error) => Some(error),
            Self::Artifact(error) => Some(error),
            Self::AttemptWorkspace(error) => Some(error),
            _ => None,
        }
    }
}

fn check_workspace(
    context: &TaskContext,
    observation_id: ObservationId,
) -> Result<Box<BoundCandidate>, ExecutionError> {
    let contract = context.contract();
    let current = inspect_candidate(
        contract.candidate().repository().root(),
        contract,
        observation_id,
    )
    .map_err(ExecutionError::Inspection)?;
    match bind_candidate(contract, context.capture(), Some(&current))
        .map_err(ExecutionError::Binding)?
    {
        BindingDecision::Compatible(binding) => Ok(binding),
        other => Err(ExecutionError::Workspace(other)),
    }
}

fn selected_effect(operation: &Operation) -> Result<(&EffectId, &EffectScope), ExecutionError> {
    if let Command::SupervisedEffect(event) = &operation.command
        && let EffectEvent::Dispatch { effect, scope, .. } = event.as_ref()
    {
        return Ok((effect, scope));
    }
    Err(ExecutionError::Dispatch)
}

/// Explicit selection of a previously reviewed direct-child fixture.
///
/// The preparation and pin bind exact original input and executable bytes.
/// The durable store and Harness retain responsibility for all effect cuts.
pub struct PreparedFixtureRun<'a> {
    /// Exact original preparation that was separately committed.
    pub preparation: &'a ExecutorPreparation,
    /// Proposed canonical effect, with explicit operation ID, grant and revision.
    pub dispatch: Operation,
    /// Exact executable and process contract pin from Harness.
    pub pin: &'a PinnedFixture,
    /// Distinct journal and process-observation identities.
    pub captures: CaptureIds,
    /// ID of the new bounded Git observation compared to the retained baseline.
    pub workspace_observation: ObservationId,
}

/// Bounded candidate correspondence and canonical process outcome are separate.
pub struct FixtureRun {
    /// Actual pre-dispatch Git observation bound by Core.
    pub workspace: Box<BoundCandidate>,
    /// Original supervisor receipts and terminal process observations.
    pub run: RunResult,
}

/// Execute one exact prepared cooperative fixture through the canonical store.
///
/// Matching Git samples cover the existing binding contract. The local workspace
/// must remain cooperative between the read-only Git observation and actual
/// launch. At the locked effect cut Harness rechecks the retained inputs and
/// pinned process/path facts. This function never rebases stale dispatches. An
/// already attempted effect belongs to canonical reconciliation.
pub fn run_prepared_fixture(
    store: &ProgramStore,
    key: &StoreKey,
    request: PreparedFixtureRun<'_>,
    cancel: &AtomicBool,
) -> Result<FixtureRun, ExecutionError> {
    run_prepared_fixture_inner(store, key, request, None, cancel)
}

fn run_prepared_fixture_inner(
    store: &ProgramStore,
    key: &StoreKey,
    request: PreparedFixtureRun<'_>,
    attempt_workspace: Option<&FixtureWorkspace>,
    cancel: &AtomicBool,
) -> Result<FixtureRun, ExecutionError> {
    if store.profile() != ProgramProfile::SupervisedFixturesV3
        || request
            .preparation
            .input()
            .requirements()
            .required()
            .iter()
            .any(|r| r.kind != ObservationKind::ControlledFixture)
    {
        return Err(ExecutionError::UnsupportedProfile);
    }

    let kernel = store
        .load(key, Access::ReadOnly)
        .map_err(ExecutionError::Store)?;
    let (effect, scope) = selected_effect(&request.dispatch)?;
    if request.dispatch.expected != kernel.revision()
        || kernel
            .operations()
            .iter()
            .any(|old| old.id == request.dispatch.id)
    {
        return Err(ExecutionError::Dispatch);
    }
    let spec = kernel
        .fixture_effects()
        .process_spec(effect)
        .ok_or(ExecutionError::PreparedInput)?;
    if spec.containment.is_some()
        || spec.identity_checkpoint.is_some()
        || spec.limits.transport.is_none()
    {
        return Err(ExecutionError::UnsupportedProfile);
    }
    let preparation = request.preparation.operation();
    if spec.prepared_input != preparation.id
        || !usize::try_from(preparation.expected)
            .ok()
            .and_then(|index| kernel.operations().get(index))
            .is_some_and(|operation| operation == preparation)
    {
        return Err(ExecutionError::PreparedInput);
    }
    let prepared = kernel
        .prepared_inputs()
        .iter()
        .find(|(id, _)| id == &preparation.id)
        .map(|(_, prepared)| prepared)
        .ok_or(ExecutionError::PreparedInput)?;
    if prepared != request.preparation.input() {
        return Err(ExecutionError::PreparedInput);
    }
    prepared
        .check(
            kernel.state(),
            prepared.descriptor(),
            prepared.requirements(),
            prepared.bytes(),
        )
        .map_err(ExecutionError::Input)?;
    let current = kernel
        .state()
        .readiness()
        .scope(&scope.task)
        .map_err(|_| ExecutionError::Dispatch)?;
    if current.context().contract().candidate() != &scope.candidate {
        return Err(ExecutionError::Dispatch);
    }
    let workspace = check_workspace(current.context(), request.workspace_observation)?;
    if let Some(attempt) = attempt_workspace {
        if attempt.store != store.id()
            || &attempt.effect != effect
            || &attempt.scope != scope
            || spec.cwd != scope.candidate.repository().root()
            || spec.args.get(attempt.output_argument).map(Path::new) != Some(attempt.output())
        {
            return Err(WorkspaceError::Correspondence.into());
        }
        attempt.check_empty()?;
    }
    drop(kernel);
    let run = execute_fixture(
        store,
        key,
        request.dispatch,
        request.captures,
        request.pin,
        cancel,
    )
    .map_err(ExecutionError::Harness)?;
    Ok(FixtureRun { workspace, run })
}

/// The one exact declared output the caller will examine.
pub struct FixtureArtifactRequest<'a> {
    /// Canonical producer effect retaining the terminal process observation.
    pub effect: &'a EffectId,
    /// Exact declared producer artifact (its ID, revision and candidate).
    pub artifact: &'a ArtifactReference,
    /// Canonical private output root.
    pub output_root: &'a Path,
    /// Explicit single output path, with no discovery of child-selected paths.
    pub path: &'a Path,
    /// Additional source repositories whose physical roots are protected.
    pub additional_sources: &'a [RepositoryIdentity],
    /// Positive capture bound within program limits.
    pub max_bytes: usize,
    /// Expected SHA-256 from an independent oracle selected by the caller.
    pub expected_sha256: &'a str,
    /// ID of a new bounded Git observation at the capture boundary.
    pub workspace_observation: ObservationId,
}

/// Captured bytes and their distinct canonical and workspace correspondences.
///
/// The caller supplies the file path and expected digest. Equal bytes could
/// have been written by another cooperating actor. A reviewer supplies an
/// independent semantic observation and separately scoped review to release an
/// edge through Evidence.
pub struct FixtureArtifact {
    /// Revision at which the producer correspondence was read.
    pub revision: u64,
    /// Core binding from the retained contract and the separate Git inspection.
    pub workspace: Box<BoundCandidate>,
    /// Actual bounded file bytes and digest, associated with the terminal
    /// observation by the caller's output selection and Harness checks.
    pub content: CapturedArtifact,
}

/// Capture an exact declared artifact after the retained process observation.
///
/// Harness protects the path and checks canonical artifact and process identity.
/// The caller's expected digest distinguishes changed bytes inside that boundary.
/// Physical file authorship remains unestablished.
pub fn capture_fixture_output(
    store: &ProgramStore,
    key: &StoreKey,
    request: FixtureArtifactRequest<'_>,
) -> Result<FixtureArtifact, ExecutionError> {
    if store.profile() != ProgramProfile::SupervisedFixturesV3 {
        return Err(ExecutionError::UnsupportedProfile);
    }
    let kernel = store
        .load(key, Access::ReadOnly)
        .map_err(ExecutionError::Store)?;
    let content = capture_artifact(
        &kernel,
        request.effect,
        request.artifact,
        request.output_root,
        request.path,
        request.additional_sources,
        request.max_bytes,
    )
    .map_err(ExecutionError::Artifact)?;
    if request.expected_sha256 != content.sha256() {
        return Err(ExecutionError::ArtifactDigest);
    }
    let scope = kernel
        .fixture_effects()
        .effects()
        .iter()
        .find(|record| &record.intent.id == request.effect)
        .ok_or(ExecutionError::Dispatch)?;
    let current = kernel
        .state()
        .readiness()
        .scope(&scope.intent.scope.task)
        .map_err(|_| ExecutionError::Dispatch)?;
    let workspace = check_workspace(current.context(), request.workspace_observation)?;
    let revision = kernel.revision();
    drop(kernel);
    if store
        .load(key, Access::ReadOnly)
        .map_err(ExecutionError::Store)?
        .revision()
        != revision
    {
        return Err(ExecutionError::ChangedRevision);
    }
    Ok(FixtureArtifact {
        revision,
        workspace,
        content,
    })
}
