use cortex_core::{
    program::{
        OperationId,
        domain::DomainIntent,
        executor::PreparedInput,
        supervision::{ProcessLimits, ScopeControls},
    },
    temporal::SourceReference,
};
use cortex_evidence::program::CoordinationKernel;
use rustix::fs::{MemfdFlags, Mode, OFlags, ResolveFlags, SealFlags};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs::File,
    io::{self, Read, Seek, SeekFrom},
    os::{
        fd::AsRawFd,
        unix::fs::{FileExt, MetadataExt},
    },
    path::{Component, Path, PathBuf},
};

/// Finite non-disclosing refusal. OS causes retain their category, never paths.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionError {
    /// Existing package, scope or current authority refused correspondence.
    Authority,
    /// A finite explicit policy value is missing, ambiguous or malformed.
    Policy,
    /// Approved bytes or file identity changed.
    Identity,
    /// Mandatory delegated enforcement has not been established.
    Delegation,
    /// Canonical independent manager lifetime/retention guarantees are absent.
    ManagerGuarantees,
    /// Actual host I/O failed.
    Io(io::ErrorKind),
}
impl std::fmt::Display for ExecutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "native execution: {self:?}")
    }
}
impl std::error::Error for ExecutionError {}
impl From<io::Error> for ExecutionError {
    fn from(e: io::Error) -> Self {
        Self::Io(e.kind())
    }
}
impl From<rustix::io::Errno> for ExecutionError {
    fn from(e: rustix::io::Errno) -> Self {
        Self::from(io::Error::from(e))
    }
}

/// An immutable file's role in the minimal child view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileRole {
    /// Exactly one approved entry executable.
    Executable,
    /// Explicit runtime/interpreter file; never an entire host directory.
    Runtime,
    /// Declared source input, mounted read-only and non-executable.
    Input,
}
/// Caller-approved exact file. Paths and bytes are deliberately absent from Debug.
#[derive(Clone, PartialEq, Eq)]
pub struct FileSpec {
    /// Explicit host file; every component must be non-symlink.
    pub source: PathBuf,
    /// Absolute destination in the private root, outside scratch.
    pub destination: PathBuf,
    /// SHA-256 of the approved bytes.
    pub sha256: [u8; 32],
    /// Positive maximum copy size, in bytes.
    pub max_bytes: u64,
    /// Selected access role.
    pub role: FileRole,
}
/// Sealed copy of the actual verified bytes. It cannot be changed through the
/// original pathname or another writable mapping. This is not execution authority.
pub struct PinnedFile {
    pub(crate) file: File,
    pub(crate) spec: FileSpec,
    len: u64,
}
impl PinnedFile {
    /// Verify a regular no-symlink file, copy within the explicit bound, hash the
    /// actual copy and install immutable seals before it can become a mount source.
    pub fn inspect(spec: &FileSpec) -> Result<Self, ExecutionError> {
        if !spec.source.is_absolute()
            || spec
                .source
                .components()
                .any(|c| !matches!(c, Component::RootDir | Component::Normal(_)))
            || spec.max_bytes == 0
            || spec.max_bytes > 64 * 1024 * 1024
        {
            return Err(ExecutionError::Policy);
        }
        destination(&spec.destination)?;
        let root = rustix::fs::open(
            "/",
            OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
        )?;
        let relative = spec
            .source
            .strip_prefix("/")
            .map_err(|_| ExecutionError::Policy)?;
        let fd = rustix::fs::openat2(
            &root,
            relative,
            OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NONBLOCK,
            Mode::empty(),
            ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_MAGICLINKS,
        )?;
        let mut original = File::from(fd);
        let before = original.metadata()?;
        if !before.is_file() || before.len() > spec.max_bytes {
            return Err(ExecutionError::Policy);
        }
        let fd = rustix::fs::memfd_create(
            "cortex-approved-file",
            MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING,
        )?;
        let mut file = File::from(fd);
        let count = io::copy(
            &mut Read::by_ref(&mut original).take(spec.max_bytes + 1),
            &mut file,
        )?;
        let after = original.metadata()?;
        if count != before.len()
            || after.len() != count
            || before.dev() != after.dev()
            || before.ino() != after.ino()
            || before.mtime() != after.mtime()
            || before.mtime_nsec() != after.mtime_nsec()
            || before.ctime() != after.ctime()
            || before.ctime_nsec() != after.ctime_nsec()
        {
            return Err(ExecutionError::Identity);
        }
        file.seek(SeekFrom::Start(0))?;
        let mut hash = Sha256::new();
        let mut buffer = [0; 8192];
        loop {
            let n = file.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            hash.update(&buffer[..n]);
        }
        if <[u8; 32]>::from(hash.finalize()) != spec.sha256 {
            return Err(ExecutionError::Identity);
        }
        let seals = SealFlags::WRITE | SealFlags::GROW | SealFlags::SHRINK | SealFlags::SEAL;
        rustix::fs::fcntl_add_seals(&file, seals)?;
        if rustix::fs::fcntl_get_seals(&file)? != seals {
            return Err(ExecutionError::Identity);
        }
        file.seek(SeekFrom::Start(0))?;
        Ok(Self {
            file,
            spec: spec.clone(),
            len: count,
        })
    }
    /// Recheck actual sealed bytes, rather than a cached length or source path.
    pub fn verify(&self) -> Result<(), ExecutionError> {
        let seals = SealFlags::WRITE | SealFlags::GROW | SealFlags::SHRINK | SealFlags::SEAL;
        if rustix::fs::fcntl_get_seals(&self.file)? != seals
            || self.file.metadata()?.len() != self.len
        {
            return Err(ExecutionError::Identity);
        }
        let mut h = Sha256::new();
        let mut offset = 0;
        let mut buffer = [0; 8192];
        loop {
            let n = self.file.read_at(&mut buffer, offset)?;
            if n == 0 {
                break;
            }
            h.update(&buffer[..n]);
            offset += n as u64;
        }
        if offset != self.len || <[u8; 32]>::from(h.finalize()) != self.spec.sha256 {
            return Err(ExecutionError::Identity);
        }
        Ok(())
    }
    /// Captured exact length.
    pub fn len(&self) -> u64 {
        self.len
    }
    /// Whether the approved file is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
    pub(crate) fn mount_source(&self) -> PathBuf {
        PathBuf::from(format!("/proc/self/fd/{}", self.file.as_raw_fd()))
    }
}
pub(crate) fn destination(path: &Path) -> Result<(), ExecutionError> {
    if !path.is_absolute()
        || path.to_str().is_none()
        || path.as_os_str().len() > 256
        || path
            .components()
            .any(|p| !matches!(p, Component::RootDir | Component::Normal(_)))
        || path == Path::new("/")
        || ["/scratch", "/old-root", "/proc", "/dev", "/sys"]
            .iter()
            .any(|p| path.starts_with(p))
    {
        return Err(ExecutionError::Policy);
    }
    Ok(())
}
/// Immutable caller-selected executable policy, using canonical resource types.
/// No environment is inherited; only named public values may be supplied.
#[derive(Clone, PartialEq, Eq)]
pub struct ExecutionSpec {
    /// Exact finite files, including exactly one executable.
    pub files: Vec<FileSpec>,
    /// Explicit UTF-8 argument vector.
    pub arguments: Vec<String>,
    /// Explicit public environment; key must be in environment_keys.
    pub environment: Vec<(String, String)>,
    /// Caller-approved keys; secret/session/loader keys remain forbidden.
    pub environment_keys: Vec<String>,
    /// Existing finite transport/process contract.
    pub limits: ProcessLimits,
    /// Mandatory installed group controls, without defaults.
    pub controls: ScopeControls,
    /// Writable scratch byte quota.
    pub scratch_bytes: u64,
    /// Writable scratch inode quota.
    pub scratch_inodes: u64,
    /// Directory inside scratch; no host current directory is adopted.
    pub working_directory: PathBuf,
    /// Explicit bounded artifact expectations.
    pub artifacts: Vec<super::artifacts::ArtifactSpec>,
}
impl ExecutionSpec {
    pub(crate) fn validate(&self) -> Result<(), ExecutionError> {
        let l = &self.limits;
        let Some(t) = &l.transport else {
            return Err(ExecutionError::Policy);
        };
        if self.files.is_empty()
            || self.files.len() > 16
            || self
                .files
                .iter()
                .filter(|f| f.role == FileRole::Executable)
                .count()
                != 1
            || l.wall_ms == 0
            || l.cleanup_ms == 0
            || l.authority_poll_ms == 0
            || l.input_bytes == 0
            || l.input_bytes > 1024 * 1024
            || l.output_bytes == 0
            || l.output_bytes > 1024 * 1024
            || t.stdout_bytes > l.output_bytes
            || t.stderr_bytes > l.output_bytes
            || t.buffer_bytes
                < l.input_bytes
                    .saturating_add(l.output_bytes)
                    .saturating_add(8192)
            || self.arguments.len() > l.arguments
            || self
                .arguments
                .iter()
                .try_fold(0usize, |n, a| n.checked_add(a.len()))
                .is_none_or(|n| n > l.argument_bytes)
            || self.arguments.iter().any(|a| a.contains('\0'))
            || self.scratch_bytes == 0
            || !self.scratch_bytes.is_multiple_of(4096)
            || self.scratch_inodes == 0
            || !self.working_directory.starts_with("/scratch")
            || self
                .working_directory
                .components()
                .any(|c| !matches!(c, Component::RootDir | Component::Normal(_)))
        {
            return Err(ExecutionError::Policy);
        }
        super::cgroup::validate_controls(&self.controls)?;
        if self.controls.tasks_max > 8 || self.controls.swap_bytes != 0 {
            return Err(ExecutionError::Policy);
        }
        let mut destinations = BTreeSet::new();
        let mut total = 0u64;
        for file in &self.files {
            destination(&file.destination)?;
            total = total
                .checked_add(file.max_bytes)
                .ok_or(ExecutionError::Policy)?;
            if file.max_bytes == 0
                || total > l.executable_bytes
                || !destinations.insert(file.destination.clone())
            {
                return Err(ExecutionError::Policy);
            }
        }
        for a in &destinations {
            for b in &destinations {
                if a != b && a.starts_with(b) {
                    return Err(ExecutionError::Policy);
                }
            }
        }
        let mut keys = BTreeSet::new();
        if self.environment.len() > 16 || self.environment_keys.len() > 16 {
            return Err(ExecutionError::Policy);
        }
        for (key, value) in &self.environment {
            if !keys.insert(key)
                || !self.environment_keys.contains(key)
                || key.is_empty()
                || key.len() > 64
                || !key
                    .bytes()
                    .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
                || value.len() > 1024
                || value.contains('\0')
                || [
                    "LD_", "DYLD_", "CORTEX_", "CODEX_", "OPENAI_", "GITHUB_", "SSH_", "AWS_",
                ]
                .iter()
                .any(|p| key.starts_with(p))
                || ["TOKEN", "SECRET", "PASSWORD", "CREDENTIAL", "SESSION"]
                    .iter()
                    .any(|p| key.contains(p))
            {
                return Err(ExecutionError::Policy);
            }
        }
        super::artifacts::validate_specs(&self.artifacts, self.scratch_bytes)?;
        Ok(())
    }
}
/// Package-correspondent immutable preparation. No launch or grant is produced.
pub struct ExecutionPlan {
    pub(crate) spec: ExecutionSpec,
    pub(crate) pins: Vec<PinnedFile>,
    prepared: PreparedInput,
    input_operation: OperationId,
    digest: String,
}
fn hash_field(h: &mut Sha256, bytes: &[u8]) {
    h.update((bytes.len() as u64).to_be_bytes());
    h.update(bytes);
}
impl ExecutionPlan {
    /// Validate the existing admitted prepared input and pin all approved bytes.
    /// Supply the exact member kernel that owns this input for registered-member
    /// preparation; the later check uses the coordinator's canonical member view.
    pub fn inspect(
        kernel: &CoordinationKernel,
        input_operation: &OperationId,
        spec: ExecutionSpec,
    ) -> Result<Self, ExecutionError> {
        spec.validate()?;
        let prepared = kernel
            .prepared_inputs()
            .iter()
            .find(|(id, _)| id == input_operation)
            .map(|(_, p)| p.clone())
            .ok_or(ExecutionError::Authority)?;
        prepared
            .check(
                kernel.state(),
                prepared.descriptor(),
                prepared.requirements(),
                prepared.bytes(),
            )
            .map_err(|_| ExecutionError::Authority)?;
        if prepared.bytes().len() as u64 > spec.limits.input_bytes {
            return Err(ExecutionError::Policy);
        }
        let pins = spec
            .files
            .iter()
            .map(PinnedFile::inspect)
            .collect::<Result<Vec<_>, _>>()?;
        let mut h = Sha256::new();
        hash_field(&mut h, b"cortex-native-executable-v1");
        hash_field(&mut h, input_operation.as_str().as_bytes());
        hash_field(&mut h, prepared.bytes());
        // Every selected policy field is represented by a deterministic JSON array.
        hash_field(&mut h, Self::policy_value(&spec).to_string().as_bytes());
        let digest = format!("{:x}", h.finalize());
        Ok(Self {
            spec,
            pins,
            prepared,
            input_operation: input_operation.clone(),
            digest,
        })
    }
    /// Exact configuration digest, suitable for the existing DomainIntent.
    pub fn digest(&self) -> &str {
        &self.digest
    }
    /// Recheck the canonical package/current domain and immutable full policy.
    /// This must run again inside the existing locked release callback.
    pub fn check(
        &self,
        kernel: &CoordinationKernel,
        intent: &DomainIntent,
        at: u64,
    ) -> Result<(), ExecutionError> {
        for pin in &self.pins {
            pin.verify()?;
        }
        kernel
            .check_domain_current(&intent.id, at)
            .map_err(|_| ExecutionError::Authority)?;
        let member = kernel
            .domain_member(&intent.effect.scope.program)
            .ok_or(ExecutionError::Authority)?;
        let current = member
            .prepared_inputs()
            .iter()
            .find(|(id, _)| id == &self.input_operation)
            .map(|(_, p)| p)
            .ok_or(ExecutionError::Authority)?;
        if current != &self.prepared {
            return Err(ExecutionError::Authority);
        }
        current
            .check(
                member.state(),
                current.descriptor(),
                current.requirements(),
                current.bytes(),
            )
            .map_err(|_| ExecutionError::Authority)?;
        let scope = &intent.effect.scope;
        let request = current.package().request();
        if request.task != scope.task
            || request.baseline != scope.baseline
            || request.generation != scope.generation
            || request.id != scope.package
            || request.revision != scope.package_revision
            || intent.configuration != self.configuration()?
        {
            return Err(ExecutionError::Authority);
        }
        Ok(())
    }
    /// Public source reference naming only the digest, never host paths or bytes.
    pub fn configuration(&self) -> Result<SourceReference, ExecutionError> {
        SourceReference::operator_reported("trusted-operator", "native-executable-v1", &self.digest)
            .map_err(|_| ExecutionError::Policy)
    }
    pub(crate) fn input(&self) -> &[u8] {
        self.prepared.bytes()
    }
    pub(crate) fn policy_value(spec: &ExecutionSpec) -> serde_json::Value {
        let l = &spec.limits;
        let c = &spec.controls;
        serde_json::json!([
            spec.files
                .iter()
                .map(|f| serde_json::json!([
                    f.source,
                    f.destination,
                    format!("{:x}", Sha256::digest(f.sha256)),
                    f.max_bytes,
                    match f.role {
                        FileRole::Executable => 0,
                        FileRole::Runtime => 1,
                        FileRole::Input => 2,
                    }
                ]))
                .collect::<Vec<_>>(),
            spec.arguments,
            spec.environment,
            spec.environment_keys,
            spec.working_directory,
            [
                l.wall_ms,
                l.cleanup_ms,
                l.input_bytes,
                l.output_bytes,
                l.frame_bytes,
                l.authority_poll_ms,
                l.executable_bytes
            ],
            [l.arguments, l.argument_bytes],
            l.transport.as_ref().map(|t| [
                t.startup_ms,
                t.idle_ms,
                t.stdout_bytes,
                t.stderr_bytes,
                t.records,
                t.buffer_bytes
            ]),
            [
                c.cpu_quota_us,
                c.cpu_period_us,
                c.memory_bytes,
                c.swap_bytes,
                c.tasks_max
            ],
            spec.scratch_bytes,
            spec.scratch_inodes,
            spec.artifacts
                .iter()
                .map(|a| serde_json::json!([a.path, a.max_bytes, a.sha256]))
                .collect::<Vec<_>>()
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::supervisor::tests::support;
    use cortex_core::program::supervision::TransportLimits;
    pub(super) fn spec() -> ExecutionSpec {
        ExecutionSpec {
            files: vec![FileSpec {
                source: "/owned/fixture".into(),
                destination: "/bin/fixture".into(),
                sha256: [1; 32],
                max_bytes: 4096,
                role: FileRole::Executable,
            }],
            arguments: vec![],
            environment: vec![],
            environment_keys: vec!["LANG".into()],
            limits: ProcessLimits {
                wall_ms: 1000,
                cleanup_ms: 1000,
                input_bytes: 4096,
                output_bytes: 4096,
                frame_bytes: 4096,
                authority_poll_ms: 1,
                executable_bytes: 4096,
                arguments: 2,
                argument_bytes: 32,
                transport: Some(TransportLimits {
                    startup_ms: 500,
                    idle_ms: 500,
                    stdout_bytes: 4096,
                    stderr_bytes: 0,
                    records: 1,
                    buffer_bytes: 16384,
                }),
            },
            controls: ScopeControls {
                cpu_quota_us: 50000,
                cpu_period_us: 100000,
                memory_bytes: 134217728,
                swap_bytes: 0,
                tasks_max: 8,
            },
            scratch_bytes: 65536,
            scratch_inodes: 64,
            working_directory: "/scratch".into(),
            artifacts: vec![],
        }
    }
    #[test]
    fn missing_prepared_package_refuses_without_opening_executable() {
        let (kernel, _, _) = support::prepared_kernel(None);
        assert!(matches!(
            ExecutionPlan::inspect(&kernel, &OperationId::new("missing-input").unwrap(), spec()),
            Err(ExecutionError::Authority)
        ));
    }
    #[test]
    fn policy_rejects_escape_overlap_ambient_secret_and_unbounded_transport() {
        let valid = spec();
        assert_eq!(valid.validate(), Ok(()));
        for dest in ["/scratch/exe", "/bin/../escape", "relative", "/proc/exe"] {
            let mut s = valid.clone();
            s.files[0].destination = dest.into();
            assert!(s.validate().is_err());
        }
        let mut s = valid.clone();
        let mut second = s.files[0].clone();
        second.destination = "/bin/fixture/nested".into();
        second.role = FileRole::Input;
        s.files.push(second);
        s.limits.executable_bytes = 8192;
        assert!(s.validate().is_err());
        for key in [
            "LD_PRELOAD",
            "GITHUB_TOKEN",
            "SESSION",
            "PRIVATE_PASSWORD",
            "NOT_ALLOWED",
        ] {
            let mut s = valid.clone();
            s.environment.push((key.into(), "synthetic-canary".into()));
            assert!(s.validate().is_err());
        }
        let mut s = valid.clone();
        s.arguments.push("x".repeat(33));
        assert!(s.validate().is_err());
        let mut s = valid.clone();
        s.limits.transport = None;
        assert!(s.validate().is_err());
        let mut s = valid;
        s.controls.tasks_max = 0;
        assert!(s.validate().is_err());
    }
    #[test]
    fn policy_identity_changes_with_each_material_field() {
        let original = spec();
        let baseline = ExecutionPlan::policy_value(&original);
        let mut variants = Vec::new();
        let mut s = original.clone();
        s.files[0].sha256[0] ^= 1;
        variants.push(s);
        let mut s = original.clone();
        s.arguments.push("different".into());
        variants.push(s);
        let mut s = original.clone();
        s.controls.memory_bytes += 4096;
        variants.push(s);
        let mut s = original.clone();
        s.working_directory = "/scratch/subdir".into();
        variants.push(s);
        let mut s = original.clone();
        s.environment.push(("LANG".into(), "C".into()));
        variants.push(s);
        let mut s = original;
        s.scratch_inodes += 1;
        variants.push(s);
        for changed in variants {
            assert_ne!(ExecutionPlan::policy_value(&changed), baseline);
        }
    }
}

#[cfg(test)]
mod byte_tests {
    use super::*;
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;
    #[test]
    fn sealed_copy_rejects_writes_and_truncation_and_preserves_actual_bytes() {
        let root = std::env::temp_dir().join(format!("cxd-pin-{}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
        let path = root.join("approved");
        std::fs::write(&path, b"approved").unwrap();
        let spec = FileSpec {
            source: path.clone(),
            destination: "/bin/approved".into(),
            sha256: Sha256::digest(b"approved").into(),
            max_bytes: 64,
            role: FileRole::Executable,
        };
        let pinned = PinnedFile::inspect(&spec).unwrap();
        let mut duplicate = pinned.file.try_clone().unwrap();
        assert_eq!(
            duplicate.write(b"changed!").unwrap_err().raw_os_error(),
            Some(1)
        );
        assert_eq!(duplicate.set_len(1).unwrap_err().raw_os_error(), Some(1));
        std::fs::write(path, b"replaced").unwrap();
        let mut actual = [0; 8];
        pinned.file.read_exact_at(&mut actual, 0).unwrap();
        assert_eq!(&actual, b"approved");
        assert_eq!(pinned.verify(), Ok(()));
    }
}

#[cfg(test)]
mod preparation_tests {
    use super::*;
    use crate::supervisor::tests::support;
    #[test]
    fn exact_existing_prepared_input_can_bind_a_nonexecuting_plan() {
        let (kernel, _, _) = support::prepared_kernel(None);
        let root = std::env::temp_dir().join(format!("cxd-plan-{}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        let path = root.join("approved");
        std::fs::write(&path, b"approved").unwrap();
        let mut spec = super::tests::spec();
        spec.files[0].source = path;
        spec.files[0].sha256 = Sha256::digest(b"approved").into();
        let (id, prepared) = kernel.prepared_inputs().first().unwrap();
        spec.limits.input_bytes = prepared.bytes().len() as u64;
        spec.limits.transport.as_mut().unwrap().buffer_bytes =
            spec.limits.input_bytes + spec.limits.output_bytes + 8192;
        let plan = ExecutionPlan::inspect(&kernel, id, spec.clone()).unwrap();
        assert_eq!(plan.input(), prepared.bytes());
        assert_eq!(plan.pins[0].verify(), Ok(()));
        let mut changed = spec;
        changed.files[0].sha256 = [0; 32];
        assert!(matches!(
            ExecutionPlan::inspect(&kernel, id, changed),
            Err(ExecutionError::Identity)
        ));
        assert_eq!(kernel.prepared_inputs().len(), 1);
    }
}
