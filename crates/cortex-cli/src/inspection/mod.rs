//! Bounded Git inspection and its CAP-CORE-002 typed adapter; no assessments.

mod parse;
mod process;

#[cfg(test)]
#[path = "../../tests/support/mod.rs"]
mod support;

use cortex_core::binding::{
    BindingError, CandidateBindingContract, CandidateObservation, Change, ComparisonBasis, Entry,
    Head, ObservationConsistency, ObservationCoverage, ObservationId, RepositoryIdentity,
    RepositoryState,
};
use std::{
    error::Error,
    ffi::{OsStr, OsString},
    fmt,
    io::{self, Write},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

const MAX_ENTRIES: usize = cortex_core::binding::MAX_OBSERVATION_ENTRIES;
const MAX_OPERATIONS: usize = 32;
const INSPECTION_TIMEOUT: Duration = Duration::from_secs(30);

/// Failure of bounded inspection or translation to the typed Core boundary.
#[derive(Debug)]
pub enum InspectionError {
    /// Input path could not be resolved.
    InvalidPath(io::Error),
    /// Input identifies a non-directory.
    NotDirectory,
    /// Git could not be executed.
    MissingGit,
    /// Filesystem, process or capture I/O failed.
    Io(io::Error),
    /// Git reported failure or diagnostics.
    GitFailed(&'static str),
    /// Repository form or host controls are unsupported.
    Unsupported(&'static str),
    /// A bounded resource was exhausted.
    Limit(&'static str),
    /// Samples or object resolution changed during inspection.
    Inconsistent,
    /// Git output did not match the supported formats.
    Malformed,
    /// Git state could not satisfy the typed observation boundary.
    InvalidObservation(BindingError),
}

impl InspectionError {
    /// Read the documented CLI exit category.
    pub fn exit_code(&self) -> u8 {
        match self {
            Self::InvalidPath(_) | Self::NotDirectory => 2,
            Self::MissingGit => 3,
            Self::Io(_) | Self::GitFailed(_) => 4,
            Self::Unsupported(_) => 5,
            Self::Limit(_) => 6,
            Self::Inconsistent | Self::Malformed | Self::InvalidObservation(_) => 7,
        }
    }
}

impl fmt::Display for InspectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPath(e) => write!(f, "cannot access repository path ({:?}); select an accessible directory with --repo", e.kind()),
            Self::NotDirectory => f.write_str("repository path must be a directory; use --repo <PATH>"),
            Self::MissingGit => f.write_str("Git is unavailable; install Git and check PATH"),
            Self::Io(e) => write!(f, "repository inspection I/O failed ({:?}); check access and Git installation", e.kind()),
            Self::GitFailed(operation) => write!(f, "Git could not complete {operation}; check repository access, configuration and local objects"),
            Self::Unsupported(form) => write!(f, "unsupported repository state: {form}; inspection is incomplete"),
            Self::Limit(kind) => write!(f, "inspection resource limit exceeded ({kind}); no complete report"),
            Self::Inconsistent => f.write_str("repository changed during inspection; retry when stable"),
            Self::InvalidObservation(_) => f.write_str("Git observation could not cross the typed binding boundary"),
            Self::Malformed => f.write_str("indeterminate Git observation; unsupported or malformed output; no complete report"),
        }
    }
}

impl Error for InspectionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidPath(e) | Self::Io(e) => Some(e),
            Self::InvalidObservation(e) => Some(e),
            _ => None,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
struct Sample {
    root: PathBuf,
    git_dir: PathBuf,
    common_dir: PathBuf,
    // Raw configuration is comparison material only. Never print it: it may
    // contain private data. No derived Debug of Sample is used in diagnostics.
    configuration: Vec<u8>,
    index: Vec<u8>,
    status: Vec<u8>,
    head: Head,
    entries: Vec<Entry>,
}

/// A completed bounded Git inspection; private mechanism data is not printed.
pub struct RepositoryObservation {
    sample: Sample,
    git_version: String,
}

struct Git {
    directory: PathBuf,
    deadline: Instant,
    operations: usize,
}

impl Git {
    fn run(
        &mut self,
        args: &[&OsStr],
        operation: &'static str,
        allow_absent: bool,
    ) -> Result<process::Captured, InspectionError> {
        self.operations += 1;
        if self.operations > MAX_OPERATIONS {
            return Err(InspectionError::Limit("Git operations"));
        }
        let mut command = process::git_command()?;
        command.current_dir(&self.directory);
        // Remove inherited repository/index/config/trace/transport overrides.
        for (key, _) in std::env::vars_os() {
            if key.to_string_lossy().starts_with("GIT_") {
                command.env_remove(key);
            }
        }
        command
            .env("LC_ALL", "C")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", null_device())
            .env("GIT_ATTR_NOSYSTEM", "1")
            .env("GIT_OPTIONAL_LOCKS", "0")
            .env("GIT_NO_LAZY_FETCH", "1")
            .env("GIT_NO_REPLACE_OBJECTS", "1")
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_ALLOW_PROTOCOL", "")
            .args(["--no-optional-locks", "--no-lazy-fetch", "--no-pager"]);
        for config in [
            "core.fsmonitor=false",
            "core.untrackedCache=false",
            "maintenance.auto=false",
            "gc.auto=0",
            "diff.external=",
            "diff.renames=false",
            "status.renames=false",
            "submodule.recurse=false",
            "protocol.allow=never",
        ] {
            command.args(["-c", config]);
        }
        command
            .arg("-c")
            .arg(format!("core.hooksPath={}", null_device()));
        command
            .arg("-c")
            .arg(format!("core.attributesFile={}", null_device()));
        command
            .arg("-c")
            .arg(format!("core.excludesFile={}", null_device()));
        command.args(args);
        let deadline = self.deadline.min(Instant::now() + process::PROCESS_TIMEOUT);
        let result = process::capture(
            &mut command,
            deadline,
            process::STDOUT_LIMIT,
            process::STDERR_LIMIT,
        )
        .map_err(|error| match error {
            InspectionError::Io(ref cause)
                if cause.kind() == io::ErrorKind::NotFound
                    && !Path::new("/usr/bin/prlimit").is_file() =>
            {
                InspectionError::Unsupported(
                    "resource limiter unavailable; install util-linux /usr/bin/prlimit",
                )
            }
            error => error,
        })?;
        if result.status.code() == Some(127) {
            return Err(InspectionError::MissingGit);
        }
        if (!result.status.success() && !(allow_absent && result.status.code() == Some(1)))
            || !result.stderr.is_empty()
        {
            // Even warnings can indicate unreadable/uninspected directories.
            return Err(InspectionError::GitFailed(operation));
        }
        Ok(result)
    }

    fn output(
        &mut self,
        args: &[&str],
        operation: &'static str,
    ) -> Result<Vec<u8>, InspectionError> {
        Ok(self
            .run(
                &args.iter().map(OsStr::new).collect::<Vec<_>>(),
                operation,
                false,
            )?
            .stdout)
    }

    fn location(&mut self, option: &str) -> Result<PathBuf, InspectionError> {
        let bytes = self.output(
            &["rev-parse", "--path-format=absolute", option],
            "repository discovery",
        )?;
        let bytes = bytes
            .strip_suffix(b"\n")
            .ok_or(InspectionError::Malformed)?;
        let path = PathBuf::from(os_string(bytes)?);
        path.canonicalize().map_err(InspectionError::Io)
    }

    fn sample(&mut self) -> Result<Sample, InspectionError> {
        let bare = self.output(
            &["rev-parse", "--is-bare-repository"],
            "repository discovery (a local Git worktree is required)",
        )?;
        match bare.as_slice() {
            b"true\n" => return Err(InspectionError::Unsupported("bare repository")),
            b"false\n" => {}
            _ => return Err(InspectionError::Malformed),
        }
        if self.output(
            &["rev-parse", "--is-inside-work-tree"],
            "worktree discovery",
        )? != b"true\n"
        {
            return Err(InspectionError::Unsupported(
                "directory outside the working tree",
            ));
        }
        let root = self.location("--show-toplevel")?;
        let git_dir = self.location("--absolute-git-dir")?;
        let common_dir = self.location("--git-common-dir")?;
        let configuration =
            self.output(&["config", "--null", "--list"], "configuration inspection")?;
        parse::configuration(&configuration)?;
        let index = self.output(
            &["ls-files", "--stage", "-v", "-z", "--full-name", "--", ":/"],
            "index inspection",
        )?;
        parse::index(&index)?;
        let status = self.output(
            &[
                "status",
                "--porcelain=v2",
                "-z",
                "--branch",
                "--no-ahead-behind",
                "--untracked-files=all",
                "--ignore-submodules=all",
                "--no-renames",
            ],
            "working-tree inspection",
        )?;
        let (head, entries) = parse::status(&status)?;
        match &head {
            Head::Attached { commit, .. } | Head::Detached { commit } => {
                if self.output(&["cat-file", "-t", "HEAD"], "HEAD object verification")?
                    != b"commit\n"
                {
                    return Err(InspectionError::Unsupported("HEAD is not a commit"));
                }
                let resolved = self.output(
                    &["rev-parse", "--verify", "HEAD^{commit}"],
                    "HEAD resolution",
                )?;
                if resolved.strip_suffix(b"\n") != Some(commit.as_str().as_bytes()) {
                    return Err(InspectionError::Inconsistent);
                }
            }
            Head::Unknown => return Err(InspectionError::Malformed),
            Head::Unborn { branch } => {
                let mut name = b"refs/heads/".to_vec();
                name.extend(branch);
                let name = os_string(&name)?;
                let result = self.run(
                    &[
                        OsStr::new("show-ref"),
                        OsStr::new("--verify"),
                        OsStr::new("--quiet"),
                        &name,
                    ],
                    "unborn branch verification",
                    true,
                )?;
                if result.status.code() != Some(1) {
                    return Err(InspectionError::Inconsistent);
                }
            }
        }
        Ok(Sample {
            root,
            git_dir,
            common_dir,
            configuration,
            index,
            status,
            head,
            entries,
        })
    }
}

fn null_device() -> &'static str {
    if cfg!(windows) { "NUL" } else { "/dev/null" }
}

fn os_string(bytes: &[u8]) -> Result<OsString, InspectionError> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        Ok(OsString::from_vec(bytes.to_vec()))
    }
    #[cfg(not(unix))]
    {
        String::from_utf8(bytes.to_vec())
            .map(OsString::from)
            .map_err(|_| InspectionError::Unsupported("non-UTF-8 Git path on this host"))
    }
}

/// Inspect a local working tree using the documented 002-A limits and controls.
pub fn inspect(path: &Path) -> Result<RepositoryObservation, InspectionError> {
    inspect_between(path, || {})
}

/// Perform a real bounded inspection for this exact binding contract and observation ID.
///
/// Call twice with increasing IDs in one producer scope, then use Core's
/// `bind_candidate`. IDs are caller assertions: this function captures actual
/// state but does not authenticate ordering, grant authority or assess software.
pub fn inspect_candidate(
    path: &Path,
    contract: &CandidateBindingContract,
    id: ObservationId,
) -> Result<CandidateObservation, InspectionError> {
    inspect(path)?.into_candidate(contract, id)
}

impl RepositoryObservation {
    /// Inspect the captured Git executable version, not a caller-supplied value.
    pub fn git_version(&self) -> &str {
        &self.git_version
    }
    /// Resolve the typed identity of this completed capture without another Git query.
    ///
    /// # Errors
    /// Returns `InvalidObservation` when the captured locations exceed Core identity bounds.
    pub fn repository_identity(&self) -> Result<RepositoryIdentity, InspectionError> {
        RepositoryIdentity::new(
            self.sample.root.clone(),
            self.sample.git_dir.clone(),
            self.sample.common_dir.clone(),
        )
        .map_err(InspectionError::InvalidObservation)
    }
    /// Consume this actual capture at the existing typed boundary; no state is recaptured.
    /// Contract and ID are assertions supplied by the integration, not authenticated facts.
    ///
    /// # Errors
    /// Returns `InvalidObservation` if captured state, paths or comparison material
    /// exceed the existing Core validation bounds.
    pub fn into_candidate(
        self,
        contract: &CandidateBindingContract,
        id: ObservationId,
    ) -> Result<CandidateObservation, InspectionError> {
        let sample = self.sample;
        let repository = RepositoryIdentity::new(sample.root, sample.git_dir, sample.common_dir)
            .map_err(InspectionError::InvalidObservation)?;
        let state = RepositoryState::new(
            Some(repository),
            sample.head,
            sample.entries,
            ObservationCoverage::GitStatus,
            ObservationConsistency::TwoMatchingSamples,
        )
        .map_err(InspectionError::InvalidObservation)?;
        // Framing prevents ambiguous concatenation. These bytes compare the producer's
        // observed config/index/status, never claim to identify uncommitted contents.
        let mut material = Vec::new();
        for part in [sample.configuration, sample.index, sample.status] {
            material.extend_from_slice(&(part.len() as u64).to_le_bytes());
            material.extend_from_slice(&part);
        }
        let profile = format!(
            "002-A-v1:{}:{}-{}",
            self.git_version,
            std::env::consts::OS,
            std::env::consts::ARCH
        );
        let basis = ComparisonBasis::new(&profile, material)
            .map_err(InspectionError::InvalidObservation)?;
        Ok(CandidateObservation::new(id, contract, state, Some(basis)))
    }
}

fn inspect_between(
    path: &Path,
    between: impl FnOnce(),
) -> Result<RepositoryObservation, InspectionError> {
    let deadline = Instant::now() + INSPECTION_TIMEOUT;
    let directory = path.canonicalize().map_err(InspectionError::InvalidPath)?;
    if !directory.is_dir() {
        return Err(InspectionError::NotDirectory);
    }
    let mut git = Git {
        directory,
        deadline,
        operations: 0,
    };
    let version = git.output(&["--version"], "Git version discovery")?;
    if version.len() > 256 {
        return Err(InspectionError::Limit("Git version"));
    }
    let git_version = std::str::from_utf8(&version)
        .map_err(|_| InspectionError::Malformed)?
        .trim_end_matches('\n')
        .to_owned();
    if !git_version.starts_with("git version ") || git_version.chars().any(char::is_control) {
        return Err(InspectionError::Malformed);
    }
    let first = git.sample()?;
    between();
    let second = git.sample()?;
    if first != second {
        return Err(InspectionError::Inconsistent);
    }
    if Instant::now() >= deadline {
        return Err(InspectionError::Limit("inspection time"));
    }
    Ok(RepositoryObservation {
        sample: second,
        git_version,
    })
}

impl RepositoryObservation {
    /// Print the existing human-readable 002-A report without exposing raw metadata.
    pub fn write(&self, output: &mut impl Write) -> io::Result<()> {
        let sample = &self.sample;
        writeln!(output, "Repository: {:?}", sample.root)?;
        writeln!(output, "Git directory: {:?}", sample.git_dir)?;
        writeln!(output, "Common Git directory: {:?}", sample.common_dir)?;
        match &sample.head {
            Head::Attached { branch, commit } => {
                writeln!(output, "HEAD: {}", commit.as_str())?;
                writeln!(output, "Branch: {}", escaped(branch))?;
            }
            Head::Detached { commit } => {
                writeln!(output, "HEAD: {}", commit.as_str())?;
                writeln!(output, "Branch: DETACHED")?;
            }
            Head::Unknown => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "HEAD was not observed",
                ));
            }
            Head::Unborn { branch } => {
                writeln!(output, "HEAD: NONE (unborn)")?;
                writeln!(output, "Branch: {} (unborn)", escaped(branch))?;
            }
        }
        let staged = sample
            .entries
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    Entry::Tracked {
                        staged: Some(_),
                        ..
                    }
                )
            })
            .count();
        let unstaged = sample
            .entries
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    Entry::Tracked {
                        unstaged: Some(_),
                        ..
                    }
                )
            })
            .count();
        let untracked = sample
            .entries
            .iter()
            .filter(|e| matches!(e, Entry::Untracked { .. }))
            .count();
        writeln!(
            output,
            "Working tree: {}",
            if sample.entries.is_empty() {
                "CLEAN (Git status coverage)"
            } else {
                "DIRTY"
            }
        )?;
        writeln!(
            output,
            "Staged: {staged}\nUnstaged: {unstaged}\nUntracked: {untracked}"
        )?;
        writeln!(
            output,
            "Coverage: tracked and non-ignored untracked paths; ignored paths excluded"
        )?;
        writeln!(
            output,
            "Consistency: two matching observations; not an atomic snapshot"
        )?;
        writeln!(
            output,
            "Candidate: HEAD identifies committed revision only; no working snapshot identity"
        )?;
        writeln!(
            output,
            "Mechanism: {}; {}-{}; inspection profile 002-A-v1",
            self.git_version,
            std::env::consts::OS,
            std::env::consts::ARCH
        )?;
        writeln!(
            output,
            "Assessment: none (inspection is not software verification)"
        )
    }
}

fn escaped(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(text) => format!("{text:?}"),
        Err(_) => format!("{:?}", os_string(bytes).unwrap_or_default()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn observed_between_sample_change_is_retryable() {
        let repo = support::Repo::new();
        repo.commit_file("tracked", "initial");
        let result = inspect_between(&repo.path, || {
            std::fs::write(repo.path.join("new"), "new").unwrap();
        });
        assert!(matches!(result, Err(InspectionError::Inconsistent)));
    }

    #[test]
    fn operation_budget_and_deadline_fail_before_execution() {
        let mut git = Git {
            directory: PathBuf::from("."),
            deadline: Instant::now(),
            operations: MAX_OPERATIONS,
        };
        assert!(matches!(
            git.output(&["--version"], "version"),
            Err(InspectionError::Limit("Git operations"))
        ));
        git.operations = 0;
        assert!(matches!(
            git.output(&["--version"], "version"),
            Err(InspectionError::Limit("Git time"))
        ));
    }
}
