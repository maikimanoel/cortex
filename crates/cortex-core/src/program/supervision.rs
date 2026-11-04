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
/// Immutable exact process intent. Environment is cleared, shell/PATH expansion
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
}
impl ProcessSpec {
    /// Validate finite, explicit shape before retention; actual paths, executable
    /// bytes and controls must still be checked at the supervisor boundary.
    pub fn validate(&self, bounds: ProgramLimits) -> Result<(), ProgramError> {
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
                {
                    return Err(ProgramError::Structure.into());
                }
            }
            ProcessCompletion::Spawned => {
                if self.process.is_none() || self.quiescence != Quiescence::Unknown {
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
    /// Mechanism result only. A missing/unknown result or incomplete input cannot
    /// become success, and this result never releases a dependency by itself.
    pub fn outcome(&self, spec: &ProcessSpec) -> Option<EffectOutcome> {
        match self.completion {
            ProcessCompletion::Spawned
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
                if self.quiescence == Quiescence::DirectChildReapedStreamsClosed {
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
