//! Bounded artifact capture for trusted, finite supervised fixtures.
//!
//! The caller declares which private output path belongs to an effect. Retained
//! process observations and byte correspondence do not authenticate that the
//! process wrote that file. Capture neither assesses its semantics nor releases
//! a dependency. The cooperating local filesystem assumptions of the supervisor
//! apply; this is not hostile same-user isolation or an atomic source snapshot.

use cortex_core::{
    binding::{ObservationId, RepositoryIdentity},
    parallel::ArtifactReference,
    program::{operational::EffectId, supervision::ProcessCompletion},
};
use cortex_evidence::program::{Command, CoordinationKernel};
use rustix::fs::{Mode, OFlags};
use sha2::{Digest, Sha256};
use std::{
    error::Error,
    fmt,
    fs::{self, File, Metadata},
    io::{self, Read},
    os::unix::fs::MetadataExt,
    path::Path,
};

/// Capture refusal, without exposing private paths, bytes or identities.
#[derive(Debug)]
pub enum ArtifactError {
    /// The exact current producer, increment or terminal observation is absent.
    Correspondence,
    /// Output overlaps a protected source/Git root or lacks private path ownership.
    Location,
    /// A finite byte limit is absent, excessive or exceeded.
    Limit,
    /// The file changed or is not a single-link regular file.
    Changed,
    /// A local operation failed. The original cause is retained.
    Io(io::Error),
}
impl fmt::Display for ArtifactError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Correspondence => "artifact capture: producer correspondence unavailable",
            Self::Location => "artifact capture: protected or nonprivate location",
            Self::Limit => "artifact capture: byte limit exceeded",
            Self::Changed => "artifact capture: file correspondence unavailable",
            Self::Io(_) => "artifact capture: local I/O failed",
        })
    }
}
impl Error for ArtifactError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            _ => None,
        }
    }
}
impl From<io::Error> for ArtifactError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

/// Exact SHA-256 of an already bounded artifact byte slice.
pub fn artifact_sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Binary SHA-256 of an already bounded artifact byte slice.
pub fn artifact_digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

/// Retained bytes with exact declared artifact/effect/observation correspondence.
/// This in-memory capture creates no second authoritative journal or assessment.
pub struct CapturedArtifact {
    artifact: ArtifactReference,
    effect: EffectId,
    observation: ObservationId,
    bytes: Vec<u8>,
    sha256: String,
}
impl CapturedArtifact {
    /// Exact artifact revision and candidate declared by the producer increment.
    pub fn artifact(&self) -> &ArtifactReference {
        &self.artifact
    }
    /// Exact retained effect, separate from task and journal operation identity.
    pub fn effect(&self) -> &EffectId {
        &self.effect
    }
    /// Actual retained successful terminal process observation used for capture.
    pub fn observation(&self) -> &ObservationId {
        &self.observation
    }
    /// Complete bounded bytes. The caller must independently evaluate them.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    /// Lowercase SHA-256 of these bytes; integrity is not authentication.
    pub fn sha256(&self) -> &str {
        &self.sha256
    }
}

#[derive(PartialEq, Eq)]
struct FileStamp {
    device: u64,
    inode: u64,
    len: u64,
    mode: u32,
    owner: u32,
    links: u64,
    modified: (i64, i64),
    changed: (i64, i64),
}
fn stamp(m: &Metadata) -> FileStamp {
    FileStamp {
        device: m.dev(),
        inode: m.ino(),
        len: m.len(),
        mode: m.mode(),
        owner: m.uid(),
        links: m.nlink(),
        modified: (m.mtime(), m.mtime_nsec()),
        changed: (m.ctime(), m.ctime_nsec()),
    }
}
fn overlaps(a: &Path, b: &Path) -> bool {
    a.starts_with(b) || b.starts_with(a)
}
fn excludes(root: &Path, repository: &RepositoryIdentity) -> Result<bool, ArtifactError> {
    for source in [
        repository.root(),
        repository.git_dir(),
        repository.common_dir(),
    ] {
        // RepositoryIdentity construction validates syntax, not filesystem
        // identity. Resolve aliases before enforcing physical source exclusion.
        if overlaps(root, &fs::canonicalize(source)?) {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Reject a selected output root overlapping retained or explicitly protected repositories.
pub fn output_root_excludes_sources(
    kernel: &CoordinationKernel,
    output_root: &Path,
    additional_sources: &[RepositoryIdentity],
) -> Result<(), ArtifactError> {
    if additional_sources.len() > kernel.state().definition().limits().entries {
        return Err(ArtifactError::Limit);
    }
    for repository in additional_sources {
        if !excludes(output_root, repository)? {
            return Err(ArtifactError::Location);
        }
    }
    for definition in std::iter::once(kernel.initial())
        .chain(std::iter::once(kernel.state().definition()))
        .chain(
            kernel
                .operations()
                .iter()
                .filter_map(|op| match &op.command {
                    Command::Amend { next, .. } => Some(next.as_ref()),
                    _ => None,
                }),
        )
    {
        for task in definition.graph().tasks() {
            if !excludes(
                output_root,
                task.context().contract().candidate().repository(),
            )? {
                return Err(ArtifactError::Location);
            }
        }
    }

    Ok(())
}

/// Capture one caller-associated output after a successful, quiescent fixture.
///
/// `output_root` must be a canonical private directory owned by this controller;
/// `path` must be a canonical private regular file beneath it. All retained
/// program source/Git roots are excluded. `additional_sources` must include other
/// relevant sources (for example Cortex's own worktree and Git common directory).
/// Sources are exact supplied identities, not automatically discovered authority.
///
/// The terminal observation is retrieved from the canonical kernel. Its original
/// scope must still match the current baseline/generation/candidate and its attempt
/// must own the increment declaring this artifact. Capture does not infer approval,
/// resource settlement, producer completion or semantic correctness from exit zero.
pub fn capture_artifact(
    kernel: &CoordinationKernel,
    effect: &EffectId,
    artifact: &ArtifactReference,
    output_root: &Path,
    path: &Path,
    additional_sources: &[RepositoryIdentity],
    max_bytes: usize,
) -> Result<CapturedArtifact, ArtifactError> {
    if max_bytes == 0 || max_bytes > kernel.state().definition().limits().bytes {
        return Err(ArtifactError::Limit);
    }
    let record = kernel
        .fixture_effects()
        .effects()
        .iter()
        .find(|r| &r.intent.id == effect)
        .ok_or(ArtifactError::Correspondence)?;
    let scope = &record.intent.scope;
    let state = kernel.state();
    let current = state
        .readiness()
        .scope(&scope.task)
        .map_err(|_| ArtifactError::Correspondence)?;
    let attempt = state
        .attempts()
        .iter()
        .find(|a| a.id == scope.attempt)
        .ok_or(ArtifactError::Correspondence)?;
    if scope.program != *state.definition().id()
        || scope.baseline != state.definition().revision()
        || scope.generation != current.generation()
        || scope.candidate != *current.context().contract().candidate()
        || attempt.baseline != scope.baseline
        || attempt.generation != scope.generation
        || !matches!(
            attempt.status,
            cortex_core::program::AttemptStatus::Claimed
                | cortex_core::program::AttemptStatus::Reported
        )
        || artifact.candidate() != scope.candidate.commit()
        || !state.definition().increments().iter().any(|i| {
            i.id == attempt.increment && i.task == scope.task && i.artifacts.contains(artifact)
        })
    {
        return Err(ArtifactError::Correspondence);
    }
    if !matches!(
        record.status,
        cortex_core::program::operational::EffectStatus::Reported
            | cortex_core::program::operational::EffectStatus::ReconciledReported
    ) {
        return Err(ArtifactError::Correspondence);
    }
    let spec = record
        .process_spec
        .as_ref()
        .ok_or(ArtifactError::Correspondence)?;
    let observation = record
        .process_observations
        .last()
        .ok_or(ArtifactError::Correspondence)?;
    if observation.spec != spec.reference
        || observation.completion
            != (ProcessCompletion::Exited {
                code: Some(0),
                signal: None,
            })
        || !observation.quiescent(spec)
        || observation.outcome(spec)
            != Some(cortex_core::program::operational::EffectOutcome::Succeeded)
    {
        return Err(ArtifactError::Correspondence);
    }
    let owner = fs::metadata("/proc/self")?.uid();
    let directory = fs::symlink_metadata(output_root)?;
    if !directory.is_dir()
        || directory.uid() != owner
        || directory.mode() & 0o077 != 0
        || fs::canonicalize(output_root)? != output_root
        || !path.starts_with(output_root)
        || path == output_root
        || fs::canonicalize(path)? != path
    {
        return Err(ArtifactError::Location);
    }
    output_root_excludes_sources(kernel, output_root, additional_sources)?;

    // NONBLOCK prevents a replacement FIFO from blocking open; NOFOLLOW rejects
    // a final symlink. Descriptor and path stamps reject cooperating replacement.
    let fd = rustix::fs::open(
        path,
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
        Mode::empty(),
    )
    .map_err(|e| ArtifactError::Io(e.into()))?;
    let mut file = File::from(fd);
    let before = file.metadata()?;
    if !before.is_file()
        || before.nlink() != 1
        || before.uid() != owner
        || before.mode() & 0o077 != 0
        || stamp(&fs::symlink_metadata(path)?) != stamp(&before)
    {
        return Err(ArtifactError::Changed);
    }
    if before.len() > max_bytes as u64 {
        return Err(ArtifactError::Limit);
    }
    let mut bytes = Vec::new();
    (&mut file)
        .take(max_bytes as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > max_bytes {
        return Err(ArtifactError::Limit);
    }
    if bytes.len() as u64 != before.len()
        || stamp(&file.metadata()?) != stamp(&before)
        || stamp(&fs::symlink_metadata(path)?) != stamp(&before)
        || fs::canonicalize(path)? != path
        || stamp(&fs::symlink_metadata(output_root)?) != stamp(&directory)
    {
        return Err(ArtifactError::Changed);
    }
    Ok(CapturedArtifact {
        artifact: artifact.clone(),
        effect: effect.clone(),
        observation: observation.id.clone(),
        sha256: artifact_sha256(&bytes),
        bytes,
    })
}
