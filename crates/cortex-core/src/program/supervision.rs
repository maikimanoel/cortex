//! Exact contracts for trusted, provider-free development fixture supervision.
//!
//! Core performs no I/O. These types do not confer host isolation, authenticated
//! provenance, descendant containment or authority to invoke an AI provider.
use super::{operational::*, *};
use crate::binding::ObservationId;
use std::path::{Component, PathBuf};

/// Explicit positive per-launch limits; no operational default is selected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessLimits {
    /// Elapsed milliseconds from the controller's fresh dispatch admission start.
    pub wall_ms: u64,
    /// Additional bounded direct-child cleanup milliseconds.
    pub cleanup_ms: u64,
    /// Maximum exact prepared input bytes.
    pub input_bytes: u64,
    /// Combined captured stdout/stderr byte bound.
    pub output_bytes: u64,
    /// Maximum buffered JSON-lines frame bytes, no greater than output_bytes.
    pub frame_bytes: u64,
    /// Finite interval between current-authority inspections.
    pub authority_poll_ms: u64,
    /// Maximum executable bytes for the supervisor's exact in-memory pin.
    pub executable_bytes: u64,
    /// Maximum explicitly supplied argument count; zero permits no arguments.
    pub arguments: usize,
    /// Maximum aggregate argument UTF-8 bytes; zero permits only empty arguments.
    pub argument_bytes: usize,
    /// Optional exact transport extension. Absence retains the original combined
    /// output/wall-only fixture contract, without selecting new implicit limits.
    pub transport: Option<TransportLimits>,
}
/// Additional fixture transport limits, included in the immutable process spec.
/// These are cooperative supervisor controls, not host CPU/RSS or kernel-queue
/// quotas. No default values are selected by this type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransportLimits {
    /// Dispatch-to-first-valid-stdout-record deadline, inside the wall deadline.
    pub startup_ms: u64,
    /// Maximum interval without actual input/output byte progress, inside wall.
    pub idle_ms: u64,
    /// Independent captured stdout byte ceiling, inside the combined ceiling.
    pub stdout_bytes: u64,
    /// Independent captured stderr byte ceiling; zero forbids captured stderr.
    pub stderr_bytes: u64,
    /// Maximum valid stdout JSON records, including a final record at EOF.
    pub records: u64,
    /// Maximum supervisor-owned input/frame/read-scratch bytes. This excludes
    /// canonical journal reconstruction, executable pins, allocator metadata,
    /// JSON parser nesting scratch/stack and kernel queues; it is not a
    /// whole-process limit. Parser metadata remains bounded by the frame size.
    pub buffer_bytes: u64,
}
/// One explicit read-only mount in the synthetic namespace profile. Actual
/// canonical files and their byte correspondence remain Harness observations.
#[derive(Clone, PartialEq, Eq)]
pub struct ReadOnlyMount {
    /// Explicit host file, never discovered by searching a user's home.
    pub source: PathBuf,
    /// Absolute file path inside the selected namespace.
    pub destination: PathBuf,
}
/// Narrow provider-free containment modes. Neither permits arbitrary executors.
#[derive(Clone, PartialEq, Eq)]
pub enum ScopeIsolation {
    /// An audited finite inert fixture used to qualify the owned scope primitive.
    /// No filesystem or network isolation is claimed by this mode.
    ScopeOnlyInertFixture,
    /// Owned synthetic canaries under an exact rootless namespace configuration.
    /// Missing byte/file/inode quotas or denials still block broader profiles.
    RootlessSyntheticCanary {
        /// Exact explicitly pinned namespace utility path.
        bubblewrap: PathBuf,
        /// Caller-supplied executable/configuration provenance.
        bubblewrap_identity: SourceReference,
        /// Complete bounded read-only file mount set.
        mounts: Vec<ReadOnlyMount>,
        /// Explicit tmpfs byte ceiling; not an independent file/inode quota.
        scratch_bytes: u64,
    },
}
/// Exact requested cgroup controls. CPU rate and cumulative CPU work are distinct.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopeControls {
    /// CPU quota microseconds per period.
    pub cpu_quota_us: u64,
    /// CPU quota period in microseconds.
    pub cpu_period_us: u64,
    /// Charged memory ceiling, not an address-space limit.
    pub memory_bytes: u64,
    /// Charged swap ceiling; zero forbids swap use in this group.
    pub swap_bytes: u64,
    /// Total contained processes and threads.
    pub tasks_max: u64,
}
/// Immutable, explicitly selected owned-scope request for an audited fixture.
/// This is data consumed by the existing authority/journal path, not a grant.
#[derive(Clone, PartialEq, Eq)]
pub struct OwnedScopeSpec {
    /// Exact request/configuration identity.
    pub reference: SourceReference,
    /// Fresh single-use unit name: `cortex-qualification-<ASCII identifier>.scope`.
    pub unit: String,
    /// Exact already-delegated user-manager cgroup root.
    pub delegated_root: PathBuf,
    /// Explicit local user-runtime directory; no session environment is adopted.
    pub runtime_dir: PathBuf,
    /// Exact scope-launch utility path.
    pub launcher: PathBuf,
    /// Exact launcher provenance, independently pinned by Harness.
    pub launcher_identity: SourceReference,
    /// Exact manager-inspection/termination utility path.
    pub manager: PathBuf,
    /// Exact manager utility provenance, independently pinned by Harness.
    pub manager_identity: SourceReference,
    /// Requested controls that must be read back before fixture input is released.
    pub controls: ScopeControls,
    /// Cumulative CPU-time sampling cutoff. This is cooperative and does not
    /// establish a hard group CPU-time limit; observed overshoot remains charged.
    pub cpu_time_ms: u64,
    /// Selected interval after graceful group termination is requested.
    pub cancellation_grace_ms: u64,
    /// Selected interval after forceful group termination is requested.
    pub escalation_ms: u64,
    /// Exact narrow scope or synthetic namespace profile.
    pub isolation: ScopeIsolation,
}
/// Retained identity of one actually observed owned scope incarnation.
/// A name, PID or filesystem path alone is insufficient for later adoption.
#[derive(Clone, PartialEq, Eq)]
pub struct ScopeIdentity {
    /// Exact requested scope reference.
    pub spec: SourceReference,
    /// Exact unit name.
    pub unit: String,
    /// Manager-provided 32-character lowercase hexadecimal invocation identity.
    pub invocation_id: String,
    /// Exact observed cgroup directory.
    pub cgroup: PathBuf,
    /// Observed cgroup device identity.
    pub device: u64,
    /// Observed nonzero cgroup inode identity.
    pub inode: u64,
}
/// Latest immutable scope facts. Missing observations never mean zero or empty.
#[derive(Clone, PartialEq, Eq)]
pub struct ScopeObservation {
    /// Exact retained incarnation, including when a later read is unavailable.
    pub identity: ScopeIdentity,
    /// Current readback of required controls; absent if not currently established.
    pub controls: Option<ScopeControls>,
    /// Actual initial direct-child incarnation/membership was checked by Harness.
    /// This is retained provenance, not authenticated permission.
    pub membership_verified: bool,
    /// All controller-owned query/signal helper children were reaped. Missing
    /// helper cleanup cannot establish complete operation quiescence.
    pub helper_quiescent: bool,
    /// Current manager runtime/termination properties match the exact specification.
    pub lifecycle_verified: bool,
    /// Current whole-group population bit; None is unknown, never empty.
    pub populated: Option<bool>,
    /// Latest retained cumulative CPU reading in microseconds, not capacity.
    pub cpu_usage_us: Option<u64>,
    /// Latest retained charged-memory peak, when available.
    pub memory_peak_bytes: Option<u64>,
    /// Latest retained process/thread peak, when available.
    pub tasks_peak: Option<u64>,
}
/// Explicit opt-in publication requirement. Existing specifications retain their
/// historical meaning when this policy is absent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentityCheckpointPolicy {
    /// Commit one current owned-scope identity observation before the first
    /// prepared input byte. The checkpoint confers no acceptance or retry right.
    BeforeInputV1,
}
/// Immutable exact process intent. Ambient environment is cleared. The narrow
/// scope-only inert profile adds only explicit manager routing metadata; rootless
/// fixtures clear that metadata again. Shell/PATH expansion
/// is unavailable, and stdin is the named unchanged prepared fixture package.
#[derive(Clone, PartialEq, Eq)]
pub struct ProcessSpec {
    /// Unique launch-spec identity and revision, never reused for another effect.
    pub reference: SourceReference,
    /// Absolute explicit executable path, revalidated by the supervisor.
    pub executable: PathBuf,
    /// Caller-declared executable identity matching the prepared descriptor.
    pub executable_identity: SourceReference,
    /// Absolute explicit working directory, revalidated by the supervisor.
    pub cwd: PathBuf,
    /// Exact arguments supplied directly, without shell interpretation.
    pub args: Vec<String>,
    /// Exact existing PrepareExecutorInput journal operation.
    pub prepared_input: OperationId,
    /// Exact prepared input length, separately bounded by ProcessLimits.
    pub input_len: u64,
    /// Exact host reference matching both policy and executor descriptor.
    pub host: SourceReference,
    /// Exact prepared requirement-profile reference.
    pub profile: SourceReference,
    /// Exact policy-selected disclosure/retention reference.
    pub privacy: SourceReference,
    /// Explicit local operator approval reference for this fixture specification.
    pub approval: SourceReference,
    /// Complete per-launch limits; installation or naming supplies no defaults.
    pub limits: ProcessLimits,
    /// Optional exact owned-scope fixture contract; absence preserves the
    /// historical trusted direct-child profile and its narrower cleanup meaning.
    pub containment: Option<OwnedScopeSpec>,
    /// Optional durable identity publication obligation, immutable with the spec.
    pub identity_checkpoint: Option<IdentityCheckpointPolicy>,
}
/// Observed child incarnation. PID alone can never identify or authorize adoption.
#[derive(Clone, PartialEq, Eq)]
pub struct ProcessIdentity {
    /// Fresh controller launch reference, exactly equal to ProcessSpec.reference.
    pub launch: SourceReference,
    /// Observed positive direct-child process identifier.
    pub pid: u32,
    /// Observed Linux process start tick; no reattachment by PID is provided.
    pub start_ticks: u64,
}
/// What the trusted fixture supervisor actually established about cleanup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quiescence {
    /// Explicit pre-spawn refusal: no fixture child was started.
    NoChildStarted,
    /// The owned direct child was reaped and inherited output streams reached EOF.
    /// This does not prove hostile descendant containment or protected revocation.
    DirectChildReapedStreamsClosed,
    /// Direct child reaped, streams closed and the exact owned scope observed
    /// empty with matching controls/incarnation. This still is a captured claim.
    OwnedScopeEmpty,
    /// Cleanup or process outcome remains unresolved; resource reservations remain.
    Unknown,
}
/// Original reason the bounded supervisor stopped or could not continue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterruptionReason {
    /// Explicit execution deadline expired.
    Timeout,
    /// No complete valid stdout record was observed before startup deadline.
    StartupTimeout,
    /// No input/output byte progress was observed within the idle deadline.
    IdleTimeout,
    /// Captured stdout exceeded its independent byte ceiling.
    StdoutLimit,
    /// Captured stderr exceeded its independent byte ceiling.
    StderrLimit,
    /// An additional valid JSON record exceeded the explicit event ceiling.
    EventLimit,
    /// The explicit supervisor transport-buffer bound could not be honored.
    BufferLimit,
    /// The child ended without the required first valid stdout record.
    StartupIncomplete,
    /// Scope identity, membership or configured control correspondence failed.
    ContainmentUnknown,
    /// Observed cumulative group CPU reached the cooperative sampling cutoff.
    CpuTimeLimit,
    /// Explicit cancellation request was observed.
    Cancelled,
    /// Captured output reached or exceeded its bound.
    OutputLimit,
    /// The fixture closed input before accepting the complete package.
    InputClosed,
    /// A required local I/O operation failed.
    Io,
    /// JSON-lines framing or parsing rejected captured output.
    MalformedOutput,
    /// A current grant, scope or policy ceased to authorize continued fixture work.
    AuthorityLost,
    /// Current authority could not be inspected; it was not presumed unchanged.
    AuthorityUnknown,
}
/// Mechanism observation, separate from engineering acceptance or integration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcessCompletion {
    /// Positive spawn observation with the exact owned child incarnation.
    Spawned,
    /// Current launcher and populated owned-scope correspondence captured before
    /// any input. Only its canonical committed record can satisfy the opt-in gate.
    IdentityVerified,
    /// Explicit observed refusal before starting a fixture child.
    /// A generic spawn error alone does not establish this condition.
    StartFailed,
    /// Observed direct-child exit; exactly one code or signal is present.
    Exited {
        /// Nonnegative Unix exit code, when normally exited.
        code: Option<i32>,
        /// Positive Unix signal, when terminated by signal.
        signal: Option<i32>,
    },
    /// Original interruption cause retained even if later cleanup succeeds.
    Interrupted {
        /// Original reason, never overwritten by a subsequent cleanup observation.
        reason: InterruptionReason,
    },
    /// Observations do not establish whether or how the process completed.
    Unknown,
}
/// An unresolved bounded manager helper. This is observation provenance, never
/// permission to reattach, signal a recovered PID or repeat a query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopeHelperIdentity {
    /// Positive observed OS process ID.
    pub pid: u32,
    /// Observed start ticks when captured; absence remains explicit uncertainty.
    pub start_ticks: Option<u64>,
}
/// Immutable captured summary. Raw output is deliberately absent; structural
/// validation checks correspondence, not the truth or independence of its source.
#[derive(Clone, PartialEq, Eq)]
pub struct ProcessObservation {
    /// Unique observation identity, separate from journal operation identity.
    pub id: ObservationId,
    /// Exact retained process-spec reference.
    pub spec: SourceReference,
    /// Exact child incarnation when captured; missing identity remains unknown.
    pub process: Option<ProcessIdentity>,
    /// Captured mechanism disposition.
    pub completion: ProcessCompletion,
    /// Separate cleanup observation, not inferred from cancellation/revocation.
    pub quiescence: Quiescence,
    /// Exact bytes successfully written to fixture input.
    pub input_bytes: u64,
    /// Captured stdout count; no raw payload is retained.
    pub stdout_bytes: u64,
    /// Captured stderr count; no raw payload is retained.
    pub stderr_bytes: u64,
    /// Number of structurally valid JSON-lines records, not semantic acceptance.
    pub structured_records: u64,
    /// Whether malformed or incomplete structured output was observed.
    pub malformed_output: bool,
    /// Observed monotonic milliseconds from this fresh dispatch admission start.
    pub elapsed_ms: u64,
    /// Exact source/mechanism observation reference supplied by the caller.
    pub basis: SourceReference,
    /// Immutable scope identity/control/usage observation, when actually known.
    /// An uncertain scope creation may legitimately have no captured identity.
    pub containment: Option<Box<ScopeObservation>>,
    /// At most one bounded controller helper can remain unresolved; no further
    /// helper is started until it is observed reaped. Empty is the legacy value.
    pub unresolved_scope_helpers: Vec<ScopeHelperIdentity>,
}
fn absolute_bounded_path(path: &std::path::Path, bounds: ProgramLimits) -> bool {
    path.is_absolute()
        && path.as_os_str().len() <= bounds.bytes
        && !path.as_os_str().as_encoded_bytes().contains(&0)
        && !path
            .components()
            .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
}
impl OwnedScopeSpec {
    /// Validate an exact bounded request without inspecting host support or
    /// manufacturing authority from paths, names or caller provenance.
    pub fn validate(
        &self,
        process: &ProcessLimits,
        bounds: ProgramLimits,
    ) -> Result<(), ProgramError> {
        let id = self
            .unit
            .strip_prefix("cortex-qualification-")
            .and_then(|s| s.strip_suffix(".scope"))
            .ok_or(ProgramError::Structure)?;
        if id.is_empty()
            || id.len() > 64
            || !id
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        {
            return Err(ProgramError::Structure);
        }
        for path in [
            &self.delegated_root,
            &self.runtime_dir,
            &self.launcher,
            &self.manager,
        ] {
            if !absolute_bounded_path(path, bounds) || path == std::path::Path::new("/") {
                return Err(ProgramError::Structure);
            }
        }
        let c = &self.controls;
        if c.cpu_quota_us == 0
            || c.cpu_period_us == 0
            || c.memory_bytes == 0
            || c.tasks_max == 0
            || self.cpu_time_ms == 0
            || self.cancellation_grace_ms == 0
            || self.escalation_ms == 0
            || self.cpu_capacity_millis().is_none()
            || self.elapsed_envelope_ms(process).is_none()
            || self.cpu_time_ms.checked_mul(1000).is_none()
        {
            return Err(ProgramError::Capacity);
        }
        if let ScopeIsolation::RootlessSyntheticCanary {
            bubblewrap,
            mounts,
            scratch_bytes,
            ..
        } = &self.isolation
        {
            if !absolute_bounded_path(bubblewrap, bounds)
                || *scratch_bytes == 0
                || mounts.is_empty()
                || mounts.len() > 16
                || mounts.len() > bounds.entries
            {
                return Err(ProgramError::Capacity);
            }
            for (i, mount) in mounts.iter().enumerate() {
                if !absolute_bounded_path(&mount.source, bounds)
                    || !absolute_bounded_path(&mount.destination, bounds)
                    || mount.destination == std::path::Path::new("/")
                    || ["/proc", "/dev", "/scratch"]
                        .iter()
                        .any(|p| mount.destination.starts_with(p))
                    || mounts[..i].iter().any(|m| {
                        m.destination.starts_with(&mount.destination)
                            || mount.destination.starts_with(&m.destination)
                    })
                {
                    return Err(ProgramError::Structure);
                }
            }
        }
        Ok(())
    }
    /// Rounded-up milli-CPU capacity required for the requested quota ratio.
    /// This is never cumulative CPU execution time.
    pub fn cpu_capacity_millis(&self) -> Option<u64> {
        let n = self.controls.cpu_quota_us.checked_mul(1000)?;
        (self.controls.cpu_period_us > 0).then(|| n.div_ceil(self.controls.cpu_period_us))
    }
    /// Complete cooperative execution and staged termination interval.
    pub fn elapsed_envelope_ms(&self, limits: &ProcessLimits) -> Option<u64> {
        limits
            .wall_ms
            .checked_add(self.cancellation_grace_ms)?
            .checked_add(self.escalation_ms)?
            .checked_add(limits.cleanup_ms)
    }
}
impl ScopeObservation {
    /// Structural correspondence only; capture and OS guarantees are not authenticated.
    pub fn validate(&self, spec: &OwnedScopeSpec) -> Result<(), ProgramError> {
        let i = &self.identity;
        if i.spec != spec.reference
            || i.unit != spec.unit
            || i.cgroup != spec.delegated_root.join(&spec.unit)
            || i.inode == 0
            || i.invocation_id.len() != 32
            || !i
                .invocation_id
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            || i.invocation_id.bytes().all(|b| b == b'0')
        {
            return Err(ProgramError::Stale);
        }
        Ok(())
    }
    /// Whether current captured readbacks and original checked membership match
    /// this exact request. This method performs no host verification or grant.
    pub fn corresponds(&self, spec: &OwnedScopeSpec) -> bool {
        self.validate(spec).is_ok()
            && self.membership_verified
            && self.helper_quiescent
            && self.lifecycle_verified
            && self.controls.as_ref() == Some(&spec.controls)
    }
}
impl ProcessSpec {
    /// Validate finite, explicit shape before retention; actual paths, executable
    /// bytes and controls must still be checked at the supervisor boundary.
    pub fn validate(&self, bounds: ProgramLimits) -> Result<(), ProgramError> {
        if self.identity_checkpoint.is_some() && self.containment.is_none() {
            return Err(ProgramError::Structure);
        }

        let l = &self.limits;
        if l.wall_ms == 0
            || l.cleanup_ms == 0
            || l.input_bytes == 0
            || l.output_bytes == 0
            || l.frame_bytes == 0
            || l.frame_bytes > l.output_bytes
            || l.authority_poll_ms == 0
            || l.authority_poll_ms > l.wall_ms
            || l.executable_bytes == 0
            || l.wall_ms.checked_add(l.cleanup_ms).is_none()
            || self.input_len == 0
            || self.input_len > l.input_bytes
            || l.input_bytes > bounds.bytes as u64
            || l.output_bytes > bounds.bytes as u64
            || l.frame_bytes > bounds.bytes as u64
            || l.executable_bytes > bounds.bytes as u64
            || l.arguments > bounds.entries
            || l.argument_bytes > bounds.bytes
            || self.args.len() > l.arguments
        {
            return Err(ProgramError::Capacity);
        }
        if let Some(t) = &l.transport
            && (t.startup_ms == 0
                || t.startup_ms > l.wall_ms
                || t.idle_ms == 0
                || t.idle_ms > l.wall_ms
                || t.stdout_bytes == 0
                || t.stdout_bytes > l.output_bytes
                || t.stderr_bytes > l.output_bytes
                || t.records == 0
                || t.records > bounds.entries as u64
                || t.buffer_bytes == 0
                || t.buffer_bytes > bounds.bytes as u64
                || self
                    .input_len
                    .checked_add(l.frame_bytes)
                    .and_then(|n| n.checked_add(8192))
                    .is_none_or(|n| n > t.buffer_bytes))
        {
            return Err(ProgramError::Capacity);
        }
        if let Some(scope) = &self.containment {
            scope.validate(l, bounds)?;
        }
        let mut bytes = 0_usize;
        for arg in &self.args {
            if arg.contains('\0') {
                return Err(ProgramError::Structure);
            }
            bytes = bytes.checked_add(arg.len()).ok_or(ProgramError::Capacity)?;
        }
        if bytes > l.argument_bytes {
            return Err(ProgramError::Capacity);
        }
        for path in [&self.executable, &self.cwd] {
            if !path.is_absolute()
                || path.as_os_str().len() > bounds.bytes
                || path.as_os_str().as_encoded_bytes().contains(&0)
                || path
                    .components()
                    .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
            {
                return Err(ProgramError::Structure);
            }
        }
        Ok(())
    }
}
impl ProcessObservation {
    /// Validate the typed observation without authenticating its capture.
    pub fn validate(&self, spec: &ProcessSpec) -> Result<(), OperationalError> {
        if self.spec != spec.reference
            || self
                .process
                .as_ref()
                .is_some_and(|p| p.launch != spec.reference || p.pid == 0 || p.start_ticks == 0)
        {
            return Err(ProgramError::Stale.into());
        }
        if self.unresolved_scope_helpers.len() > 1
            || (!self.unresolved_scope_helpers.is_empty() && spec.containment.is_none())
            || self
                .unresolved_scope_helpers
                .iter()
                .any(|h| h.pid == 0 || h.start_ticks == Some(0))
        {
            return Err(ProgramError::Structure.into());
        }
        match (&spec.containment, &self.containment) {
            (Some(expected), Some(actual)) => actual.validate(expected)?,
            (None, Some(_)) => return Err(ProgramError::Structure.into()),
            _ => {}
        }
        if (spec.containment.is_some()
            && self.quiescence == Quiescence::DirectChildReapedStreamsClosed)
            || (self.quiescence == Quiescence::OwnedScopeEmpty && !self.quiescent(spec))
        {
            return Err(ProgramError::Structure.into());
        }
        if self.input_bytes > spec.input_len
            || self.stdout_bytes.checked_add(self.stderr_bytes).is_none()
            || self.structured_records > self.stdout_bytes
        {
            return Err(ProgramError::Structure.into());
        }
        match &self.completion {
            ProcessCompletion::StartFailed => {
                if self.process.is_some()
                    || self.quiescence != Quiescence::NoChildStarted
                    || self.input_bytes != 0
                    || self.stdout_bytes != 0
                    || self.stderr_bytes != 0
                    || self.structured_records != 0
                    || self.malformed_output
                    || self.containment.is_some()
                    || !self.unresolved_scope_helpers.is_empty()
                {
                    return Err(ProgramError::Structure.into());
                }
            }
            ProcessCompletion::Spawned => {
                if self.process.is_none()
                    || self.quiescence != Quiescence::Unknown
                    || (spec.identity_checkpoint.is_some() && self.input_bytes != 0)
                {
                    return Err(ProgramError::Structure.into());
                }
            }
            ProcessCompletion::IdentityVerified => {
                if spec.identity_checkpoint != Some(IdentityCheckpointPolicy::BeforeInputV1)
                    || self.process.is_none()
                    || self.input_bytes != 0
                    || self.quiescence != Quiescence::Unknown
                    || !self.unresolved_scope_helpers.is_empty()
                    || self
                        .containment
                        .as_ref()
                        .zip(spec.containment.as_ref())
                        .is_none_or(|(actual, expected)| {
                            !actual.corresponds(expected) || actual.populated != Some(true)
                        })
                {
                    return Err(ProgramError::Structure.into());
                }
            }
            ProcessCompletion::Exited { code, signal } => {
                if self.process.is_none()
                    || code.is_some() == signal.is_some()
                    || code.is_some_and(|v| !(0..=255).contains(&v))
                    || signal.is_some_and(|v| v <= 0)
                    || self.quiescence == Quiescence::NoChildStarted
                {
                    return Err(ProgramError::Structure.into());
                }
            }
            ProcessCompletion::Interrupted { .. } | ProcessCompletion::Unknown => {
                if self.quiescence == Quiescence::NoChildStarted
                    || (self.process.is_none() && self.quiescence != Quiescence::Unknown)
                {
                    return Err(ProgramError::Structure.into());
                }
            }
        }
        Ok(())
    }
    /// Required cleanup correspondence for this profile. Direct-child cleanup
    /// can never stand in for an exact empty owned scope.
    pub fn quiescent(&self, spec: &ProcessSpec) -> bool {
        if !self.unresolved_scope_helpers.is_empty() {
            return false;
        }
        match &spec.containment {
            None => self.quiescence == Quiescence::DirectChildReapedStreamsClosed,
            Some(expected) => {
                self.quiescence == Quiescence::OwnedScopeEmpty
                    && self.containment.as_ref().is_some_and(|actual| {
                        actual.corresponds(expected) && actual.populated == Some(false)
                    })
            }
        }
    }
    /// Mechanism result only. A missing/unknown result or incomplete input cannot
    /// become success, and this result never releases a dependency by itself.
    pub fn outcome(&self, spec: &ProcessSpec) -> Option<EffectOutcome> {
        match self.completion {
            ProcessCompletion::Spawned
            | ProcessCompletion::IdentityVerified
            | ProcessCompletion::StartFailed
            | ProcessCompletion::Unknown => None,
            ProcessCompletion::Interrupted { .. } => Some(EffectOutcome::Interrupted),
            ProcessCompletion::Exited {
                code: Some(0),
                signal: None,
            } if self.input_bytes == spec.input_len
                && !self.malformed_output
                && self
                    .stdout_bytes
                    .checked_add(self.stderr_bytes)
                    .is_some_and(|n| n <= spec.limits.output_bytes)
                && self.elapsed_ms <= spec.limits.wall_ms =>
            {
                if spec.limits.transport.as_ref().is_some_and(|t| {
                    self.stdout_bytes > t.stdout_bytes
                        || self.stderr_bytes > t.stderr_bytes
                        || self.structured_records == 0
                        || self.structured_records > t.records
                }) {
                    return Some(EffectOutcome::Failed);
                }
                if let Some(expected) = &spec.containment {
                    let actual = self.containment.as_ref()?;
                    let (cpu, memory, tasks) = (
                        actual.cpu_usage_us?,
                        actual.memory_peak_bytes?,
                        actual.tasks_peak?,
                    );
                    if cpu > expected.cpu_time_ms.saturating_mul(1000)
                        || memory > expected.controls.memory_bytes
                        || tasks > expected.controls.tasks_max
                    {
                        return Some(EffectOutcome::Failed);
                    }
                }
                if self.quiescent(spec) {
                    Some(EffectOutcome::Succeeded)
                } else {
                    // Exit zero is retained as a fact, but cleanup and its
                    // final measured quantities are not yet established.
                    None
                }
            }
            ProcessCompletion::Exited { .. } => Some(EffectOutcome::Failed),
        }
    }
}
