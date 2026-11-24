//! Private, once-created output workspaces for the cooperative fixture profile.

use super::{
    ExecutionError, FixtureArtifact, FixtureArtifactRequest, FixtureRun, PreparedFixtureRun,
    capture_fixture_output, check_workspace, run_prepared_fixture_inner,
};
use cortex_core::{
    binding::{ObservationId, RepositoryIdentity, WorktreeRequirement},
    parallel::ArtifactReference,
    program::operational::{EffectId, EffectScope, EffectStatus},
};
use cortex_harness::{RunResult, artifact::output_root_excludes_sources};
use cortex_storage::{
    history_store::{Access, StoreId, StoreKey},
    program_store::{ProgramProfile, ProgramStore},
};
use std::{
    fs::{self, DirBuilder, File, Metadata, OpenOptions},
    io,
    os::{
        fd::AsRawFd,
        unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt},
    },
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};

/// The single output path assigned to this cooperative profile.
const OUTPUT_NAME: &str = "output";

/// Refusals at private workspace and retained artifact boundaries.
#[derive(Debug)]
pub enum WorkspaceError {
    /// The exact reserved effect, current scope or workspace role differs.
    Correspondence,
    /// The selected output root overlaps a protected repository or changed.
    Location,
    /// The fresh attempt slot already exists or contains unexpected entries.
    Occupied,
    /// The inherited cooperative deadline expired before slot creation.
    Deadline,
    /// No matching durably retained terminal observation is available.
    TerminalObservation,
    /// Local filesystem failure, reported without private paths.
    Io(io::ErrorKind),
}

impl std::fmt::Display for WorkspaceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Correspondence => {
                f.write_str("fixture workspace: effect or output role mismatch")
            }
            Self::Location => f.write_str("fixture workspace: private directory unavailable"),
            Self::Occupied => f.write_str("fixture workspace: output slot occupied"),
            Self::Deadline => f.write_str("fixture workspace: preparation deadline reached"),
            Self::TerminalObservation => {
                f.write_str("fixture workspace: terminal receipt unavailable")
            }
            Self::Io(kind) => write!(f, "fixture workspace: local I/O failed ({kind:?})"),
        }
    }
}
impl std::error::Error for WorkspaceError {}
impl From<io::Error> for WorkspaceError {
    fn from(value: io::Error) -> Self {
        Self::Io(value.kind())
    }
}
impl From<WorkspaceError> for ExecutionError {
    fn from(value: WorkspaceError) -> Self {
        Self::AttemptWorkspace(value)
    }
}

/// Exact reserved attempt and declared cooperative fixture output role.
pub struct FixtureWorkspaceRequest<'a> {
    /// Existing reserved effect for the current scoped attempt.
    pub effect: &'a EffectId,
    /// Expected current journal revision.
    pub expected_revision: u64,
    /// Canonical private directory outside protected repositories.
    pub parent: &'a Path,
    /// Other protected repositories, for example the controller source.
    pub additional_sources: &'a [RepositoryIdentity],
    /// Zero-based index of the declared output path in the exact fixture args.
    pub output_argument: usize,
    /// Separate current observation of the accepted source candidate.
    pub source_observation: ObservationId,
}

/// Fresh private output custody, bound to one exact store and reservation.
///
/// This creates no source checkout or isolation backend. The accepted clean
/// source remains a separately inspected checkout. The slot is intentionally
/// retained on error or drop so uncertainty cannot silently transfer to a
/// fresh effect.
pub struct FixtureWorkspace {
    pub(super) store: StoreId,
    pub(super) effect: EffectId,
    pub(super) scope: EffectScope,
    pub(super) output_argument: usize,
    root: PathBuf,
    output: PathBuf,
    directory: File,
    additional_sources: Vec<RepositoryIdentity>,
}

/// A bounded execution and its exact attempt workspace.
pub struct WorkspaceFixtureRun {
    workspace: FixtureWorkspace,
    run: FixtureRun,
}

fn inode(metadata: &Metadata) -> (u64, u64) {
    (metadata.dev(), metadata.ino())
}

fn private_directory(path: &Path, file: &File) -> Result<(), WorkspaceError> {
    let named = fs::symlink_metadata(path)?;
    if !named.is_dir()
        || named.uid() != fs::metadata("/proc/self")?.uid()
        || named.mode() & 0o777 != 0o700
        || fs::canonicalize(path)? != path
        || inode(&named) != inode(&file.metadata()?)
    {
        return Err(WorkspaceError::Location);
    }
    Ok(())
}

fn open_directory(path: &Path) -> Result<File, WorkspaceError> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW)
        .open(path)?;
    private_directory(path, &file)?;
    Ok(file)
}

fn directory_reference(file: &File) -> PathBuf {
    PathBuf::from(format!("/proc/self/fd/{}", file.as_raw_fd()))
}

fn create_slot(parent: &File, name: &str) -> Result<File, WorkspaceError> {
    let anchored = directory_reference(parent).join(name);
    match DirBuilder::new().mode(0o700).create(&anchored) {
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            return Err(WorkspaceError::Occupied);
        }
        result => result?,
    }
    Ok(OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW)
        .open(anchored)?)
}

impl FixtureWorkspace {
    /// The canonical, fixed slot root for the attempt.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Expected selected output path for the declared process argument.
    pub fn output(&self) -> &Path {
        &self.output
    }

    pub(super) fn check_empty(&self) -> Result<(), WorkspaceError> {
        private_directory(&self.root, &self.directory)?;
        if fs::read_dir(directory_reference(&self.directory))?
            .next()
            .is_some()
        {
            return Err(WorkspaceError::Occupied);
        }
        Ok(())
    }

    fn check_output_slot(&self) -> Result<(), WorkspaceError> {
        private_directory(&self.root, &self.directory)?;
        let mut entries = fs::read_dir(directory_reference(&self.directory))?;
        if entries
            .next()
            .transpose()?
            .is_none_or(|entry| entry.file_name() != OUTPUT_NAME)
            || entries.next().is_some()
        {
            return Err(WorkspaceError::Occupied);
        }
        Ok(())
    }
}

/// Prepare an exclusive private output slot after checking the existing reservation.
///
/// The parent must be private and separately selected before preparation. Slot
/// creation is relative to its opened directory. A preexisting slot is refused
/// even when empty. The reservation and candidate are rechecked at dispatch.
pub fn prepare_fixture_workspace(
    store: &ProgramStore,
    key: &StoreKey,
    request: FixtureWorkspaceRequest<'_>,
) -> Result<FixtureWorkspace, ExecutionError> {
    prepare_fixture_workspace_before(store, key, request, None)
}

pub(crate) fn prepare_fixture_workspace_before(
    store: &ProgramStore,
    key: &StoreKey,
    request: FixtureWorkspaceRequest<'_>,
    end: Option<std::time::Instant>,
) -> Result<FixtureWorkspace, ExecutionError> {
    if store.profile() != ProgramProfile::SupervisedFixturesV3 {
        return Err(ExecutionError::UnsupportedProfile);
    }
    let kernel = store
        .load(key, Access::ReadOnly)
        .map_err(ExecutionError::Store)?;
    let effect = kernel
        .fixture_effects()
        .effects()
        .iter()
        .find(|record| {
            &record.intent.id == request.effect
                && record.status == EffectStatus::Reserved
                && record.dispatch_at.is_none()
        })
        .ok_or(WorkspaceError::Correspondence)?;
    let scope = &effect.intent.scope;
    let current = kernel
        .state()
        .readiness()
        .scope(&scope.task)
        .map_err(|_| WorkspaceError::Correspondence)?;
    if kernel.revision() != request.expected_revision
        || &scope.program != kernel.state().definition().id()
        || scope.baseline != kernel.state().definition().revision()
        || scope.generation != current.generation()
        || &scope.candidate != current.context().contract().candidate()
        || scope.candidate.worktree() != WorktreeRequirement::Clean
    {
        return Err(WorkspaceError::Correspondence.into());
    }
    check_workspace(current.context(), request.source_observation)?;

    let parent = open_directory(request.parent)?;
    let name = request.effect.as_str();
    let root = request.parent.join(name);
    output_root_excludes_sources(&kernel, &root, request.additional_sources)
        .map_err(ExecutionError::Artifact)?;
    if end.is_some_and(|end| std::time::Instant::now() >= end) {
        return Err(WorkspaceError::Deadline.into());
    }
    let directory = create_slot(&parent, name)?;
    let workspace = FixtureWorkspace {
        store: store.id(),
        effect: request.effect.clone(),
        scope: scope.clone(),
        output_argument: request.output_argument,
        output: root.join(OUTPUT_NAME),
        root,
        directory,
        additional_sources: request.additional_sources.to_vec(),
    };
    workspace.check_empty()?;
    Ok(workspace)
}

/// Execute the work against the selected attempt's fresh output slot.
///
/// The exact retained executable specification must name this output path in its
/// declared argument. The regular cooperative supervisor admits and rechecks
/// the actual process. Filesystem identities are observational under the
/// cooperating local owner assumption.
pub fn run_workspace_fixture(
    store: &ProgramStore,
    key: &StoreKey,
    workspace: FixtureWorkspace,
    request: PreparedFixtureRun<'_>,
    cancel: &AtomicBool,
) -> Result<WorkspaceFixtureRun, ExecutionError> {
    run_workspace_fixture_inner(store, key, workspace, request, None, cancel)
}

pub(crate) fn run_workspace_fixture_inner(
    store: &ProgramStore,
    key: &StoreKey,
    workspace: FixtureWorkspace,
    request: PreparedFixtureRun<'_>,
    end: Option<std::time::Instant>,
    cancel: &AtomicBool,
) -> Result<WorkspaceFixtureRun, ExecutionError> {
    let run = run_prepared_fixture_inner(store, key, request, Some(&workspace), end, cancel)?;
    Ok(WorkspaceFixtureRun { workspace, run })
}

impl WorkspaceFixtureRun {
    /// The exact bounded direct-child run and canonical effect receipts.
    pub fn run(&self) -> &RunResult {
        &self.run.run
    }

    /// Actual compatible workspace and canonical process result.
    pub fn fixture(&self) -> &FixtureRun {
        &self.run
    }

    /// Preserved private attempt slot. Cleanup is an explicit operator action.
    pub fn workspace(&self) -> &FixtureWorkspace {
        &self.workspace
    }

    /// Capture the sole expected output against an independent byte oracle.
    ///
    /// Capture requires the exact fresh dispatch and durably retained terminal
    /// observation from this execution. Cooperative actors could still write
    /// the same bytes between observations. Semantic assessment and release
    /// remain separate canonical operations.
    pub fn capture(
        &self,
        store: &ProgramStore,
        key: &StoreKey,
        artifact: &ArtifactReference,
        expected_sha256: &str,
        max_bytes: usize,
        source_observation: ObservationId,
    ) -> Result<FixtureArtifact, ExecutionError> {
        if self.workspace.store != store.id()
            || self.run.run.dispatch.duplicate
            || self.run.run.recorded.is_none()
        {
            return Err(WorkspaceError::TerminalObservation.into());
        }
        let expected = self
            .run
            .run
            .observation
            .as_ref()
            .ok_or(WorkspaceError::TerminalObservation)?;
        self.workspace.check_output_slot()?;
        let capture = capture_fixture_output(
            store,
            key,
            FixtureArtifactRequest {
                effect: &self.workspace.effect,
                artifact,
                output_root: self.workspace.root(),
                path: self.workspace.output(),
                additional_sources: &self.workspace.additional_sources,
                max_bytes,
                expected_sha256,
                workspace_observation: source_observation,
            },
        )?;
        self.workspace.check_output_slot()?;
        if capture.content.observation() != &expected.id {
            return Err(WorkspaceError::TerminalObservation.into());
        }
        Ok(capture)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        os::unix::fs::symlink,
        sync::atomic::{AtomicUsize, Ordering},
    };

    struct Tree(PathBuf);

    impl Tree {
        fn private() -> Self {
            static SEQUENCE: AtomicUsize = AtomicUsize::new(0);
            let path = std::env::temp_dir().canonicalize().unwrap().join(format!(
                "cortex-attempt-slot-{}-{}",
                std::process::id(),
                SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ));
            DirBuilder::new().mode(0o700).create(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for Tree {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    #[test]
    fn slot_refuses_preexisting_files_and_directories() {
        let tree = Tree::private();
        let parent = open_directory(&tree.0).unwrap();
        let child = create_slot(&parent, "attempt-a").unwrap();
        private_directory(&tree.0.join("attempt-a"), &child).unwrap();
        assert!(
            fs::read_dir(tree.0.join("attempt-a"))
                .unwrap()
                .next()
                .is_none()
        );

        assert!(matches!(
            create_slot(&parent, "attempt-a"),
            Err(WorkspaceError::Occupied)
        ));
        let alternate = tree.0.join("attempt-b");
        fs::write(&alternate, b"preexisting worker bytes").unwrap();
        assert!(matches!(
            create_slot(&parent, "attempt-b"),
            Err(WorkspaceError::Occupied)
        ));
        assert_eq!(fs::read(alternate).unwrap(), b"preexisting worker bytes");
    }

    #[test]
    fn slot_pin_refuses_replacement_and_symlink() {
        let tree = Tree::private();
        let parent = open_directory(&tree.0).unwrap();
        let child = create_slot(&parent, "attempt-a").unwrap();
        let original = tree.0.join("attempt-a");
        let moved = tree.0.join("original");
        fs::rename(&original, &moved).unwrap();
        DirBuilder::new().mode(0o700).create(&original).unwrap();
        assert!(matches!(
            private_directory(&original, &child),
            Err(WorkspaceError::Location)
        ));
        assert!(fs::read_dir(moved).unwrap().next().is_none());

        symlink(&original, tree.0.join("alias")).unwrap();
        assert!(open_directory(&tree.0.join("alias")).is_err());
        assert!(matches!(
            create_slot(&parent, "alias"),
            Err(WorkspaceError::Occupied)
        ));
    }
}
