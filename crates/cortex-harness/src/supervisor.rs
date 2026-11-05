use crate::owned_scope::{PreparedScope, ScopeSignal};
use cortex_core::{
    binding::ObservationId,
    program::{
        OperationId, ProgramError, ProgramLimits,
        executor::{Control, ExecutorDescriptor, PreparedInput, Requirements, negotiate},
        operational::{EffectEvent, EffectId, EffectScope},
        scheduling::Resource,
        supervision::*,
    },
    temporal::SourceReference,
};
use cortex_evidence::program::{Command as WorkflowCommand, CoordinationKernel, Operation};
use cortex_storage::{
    history_store::{Access, Completion, StoreKey},
    program_store::{AdmissionFailure, ProgramStore, ProgramStoreError, Receipt, Reconciliation},
};
use std::{
    error::Error,
    fmt,
    fs::{self, File, Metadata},
    io::{self, Read, Write},
    net::Shutdown,
    os::{
        fd::{AsRawFd, OwnedFd},
        unix::{fs::MetadataExt, net::UnixStream, process::ExitStatusExt},
    },
    path::{Path, PathBuf},
    process::{Child, Command, ExitStatus, Stdio},
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

const POLL: Duration = Duration::from_millis(2);

/// Explicit capture identities. These records are mechanism observations, never
/// independent acceptance. Retry bounds govern only proven-stale journal appends.
#[derive(Clone)]
pub struct CaptureIds {
    /// Unique journal ID for the positive spawn observation.
    pub started_operation: OperationId,
    /// Unique journal ID for the final process observation.
    pub finished_operation: OperationId,
    /// Unique positive-spawn observation identity.
    pub started_observation: ObservationId,
    /// Unique final observation identity.
    pub finished_observation: ObservationId,
    /// Explicit observation mechanism/version reference.
    pub basis: SourceReference,
    /// Positive finite append attempts; an uncertain commit is never blindly retried.
    pub admission_attempts: usize,
}

/// Explicit identities for the opt-in pre-input publication. These are distinct
/// from spawn/final receipts and confer no scope, grant or execution authority.
#[derive(Clone)]
pub struct IdentityCaptureIds {
    /// Fresh canonical journal operation for this exact checkpoint.
    pub checkpoint_operation: OperationId,
    /// Fresh observation identity for the captured launcher/scope correspondence.
    pub checkpoint_observation: ObservationId,
}

/// An executed or replayed dispatch and its retained observations. No result here
/// reconciles resource reservations or releases an engineering dependency.
pub struct RunResult {
    /// Exact durable dispatch receipt; duplicates performed no launch.
    pub dispatch: Receipt,
    /// Positive spawn observation receipt, when durably retained.
    pub started: Option<Receipt>,
    /// Committed opt-in identity checkpoint, if its exact receipt was confirmed.
    pub identity_checkpoint: Option<Receipt>,
    /// Final observation, including unknown or interrupted cleanup.
    pub observation: Option<ProcessObservation>,
    /// Final observation receipt, when durably retained.
    pub recorded: Option<Receipt>,
    /// Original recording failure; an unrecorded observation must be reconciled.
    pub recording_failure: Option<AdmissionFailure>,
    /// Exact rejected stale-revision appends, retained before any bounded repair.
    pub stale_receipt_attempts: Vec<Operation>,
    /// Original uncertain append failures resolved by exact durable reconciliation.
    pub reconciled_receipt_failures: Vec<ReconciledReceipt>,
}

/// An original uncertain receipt append and the later exact matching observation.
pub struct ReconciledReceipt {
    /// Complete attempted journal input; its identity and payload were not changed.
    pub operation: Operation,
    /// Original uncertainty, preserved rather than relabeled as success.
    pub failure: AdmissionFailure,
    /// Later exact reconciliation of that same operation.
    pub receipt: Receipt,
}

/// Private-input-safe boundary refusal. An error before dispatch confers no
/// launch authority. Post-dispatch uncertainty is returned as a `RunResult`.
#[derive(Debug)]
pub enum HarnessError {
    /// A complete, current supervised fixture contract was not supplied.
    Contract,
    /// A required control is outside this trusted direct-child support profile.
    Unsupported(Control),
    /// Explicit bounds were missing or could not bound the selected store reads.
    Limits,
    /// Executable or directory correspondence changed or was unsupported.
    TargetChanged,
    /// A local operation failed; no private path, argument or output is exposed.
    Io(io::ErrorKind),
    /// Read-only canonical state could not be inspected.
    Store(ProgramStoreError),
    /// Durable dispatch refused or returned uncertain completion; nothing launched.
    Admission(AdmissionFailure),
}
impl fmt::Display for HarnessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Store(_) => f.write_str("fixture supervisor: canonical state unavailable"),
            Self::Admission(_) => f.write_str("fixture supervisor: dispatch admission unavailable"),
            other => write!(f, "fixture supervisor: {other:?}"),
        }
    }
}
impl Error for HarnessError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Store(e) => Some(e),
            Self::Admission(e) => Some(e),
            _ => None,
        }
    }
}
impl From<io::Error> for HarnessError {
    fn from(value: io::Error) -> Self {
        Self::Io(value.kind())
    }
}

#[derive(Clone, PartialEq, Eq)]
struct Stamp {
    dev: u64,
    ino: u64,
    len: u64,
    mode: u32,
    uid: u32,
    mtime: (i64, i64),
    ctime: (i64, i64),
}
fn stamp(m: &Metadata) -> Stamp {
    Stamp {
        dev: m.dev(),
        ino: m.ino(),
        len: m.len(),
        mode: m.mode(),
        uid: m.uid(),
        mtime: (m.mtime(), m.mtime_nsec()),
        ctime: (m.ctime(), m.ctime_nsec()),
    }
}
pub(crate) struct PinnedExecutable {
    path: PathBuf,
    file: File,
    stamp: Stamp,
    bytes: Vec<u8>,
}
impl PinnedExecutable {
    pub(crate) fn inspect(path: &Path, max: u64) -> Result<Self, HarnessError> {
        let metadata = fs::symlink_metadata(path)?;
        if !metadata.is_file()
            || metadata.mode() & 0o111 == 0
            || fs::canonicalize(path)? != path
            || max == 0
            || metadata.len() == 0
            || metadata.len() > max
        {
            return Err(HarnessError::TargetChanged);
        }
        let mut file = File::open(path)?;
        if stamp(&file.metadata()?) != stamp(&metadata) {
            return Err(HarnessError::TargetChanged);
        }
        let mut bytes = Vec::new();
        (&mut file)
            .take(max.checked_add(1).ok_or(HarnessError::Limits)?)
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 != metadata.len()
            || !bytes.starts_with(b"\x7fELF")
            || stamp(&file.metadata()?) != stamp(&metadata)
        {
            return Err(HarnessError::TargetChanged);
        }
        Ok(Self {
            path: path.to_path_buf(),
            file,
            stamp: stamp(&metadata),
            bytes,
        })
    }
    pub(crate) fn check(&self) -> Result<(), HarnessError> {
        if stamp(&fs::symlink_metadata(&self.path)?) != self.stamp
            || stamp(&self.file.metadata()?) != self.stamp
            || fs::canonicalize(&self.path)? != self.path
        {
            return Err(HarnessError::TargetChanged);
        }
        let mut file = File::open(&self.path)?;
        if stamp(&file.metadata()?) != self.stamp {
            return Err(HarnessError::TargetChanged);
        }
        let mut offset = 0_usize;
        let mut chunk = [0_u8; 8192];
        loop {
            let n = file.read(&mut chunk)?;
            if n == 0 {
                break;
            }
            let end = offset.checked_add(n).ok_or(HarnessError::Limits)?;
            if self.bytes.get(offset..end) != Some(&chunk[..n]) {
                return Err(HarnessError::TargetChanged);
            }
            offset = end;
        }
        if offset != self.bytes.len() || stamp(&file.metadata()?) != self.stamp {
            return Err(HarnessError::TargetChanged);
        }
        Ok(())
    }
}
/// An opaque, non-cloneable, read-only pin of one exact owned fixture specification.
/// It is not permission; only the canonical store can admit a launch.
pub struct PinnedFixture {
    spec: ProcessSpec,
    descriptor: ExecutorDescriptor,
    requirements: Requirements,
    input: Vec<u8>,
    executable: PinnedExecutable,
    cwd: Stamp,
    scope: Mutex<Option<PreparedScope>>,
}
fn spec_for<'a>(
    kernel: &'a CoordinationKernel,
    effect: &EffectId,
) -> Result<(&'a ProcessSpec, &'a PreparedInput), HarnessError> {
    let spec = kernel
        .fixture_effects()
        .process_spec(effect)
        .ok_or(HarnessError::Contract)?;
    let input = kernel
        .prepared_inputs()
        .iter()
        .find(|(id, _)| id == &spec.prepared_input)
        .map(|(_, p)| p)
        .ok_or(HarnessError::Contract)?;
    Ok((spec, input))
}
impl PinnedFixture {
    fn check(&self, kernel: &CoordinationKernel, effect: &EffectId) -> Result<(), HarnessError> {
        let (spec, input) = spec_for(kernel, effect)?;
        if spec != &self.spec
            || input.descriptor() != &self.descriptor
            || input.requirements() != &self.requirements
            || input.bytes() != self.input
            || stamp(&fs::symlink_metadata(&spec.cwd)?) != self.cwd
        {
            return Err(HarnessError::TargetChanged);
        }
        input
            .check(
                kernel.state(),
                input.descriptor(),
                input.requirements(),
                input.bytes(),
            )
            .map_err(|_| HarnessError::Contract)?;
        self.executable.check()
    }
}

/// Inspect and pin one exact trusted fixture before retaining its process spec.
/// This reads only explicit local files; it grants no launch authority. A fresh
/// pin after restart cannot authorize retry of an uncertain dispatch.
pub fn inspect_fixture(
    spec: &ProcessSpec,
    input: &PreparedInput,
    bounds: ProgramLimits,
) -> Result<PinnedFixture, HarnessError> {
    spec.validate(bounds).map_err(|_| HarnessError::Contract)?;
    if negotiate(input.descriptor(), input.requirements())
        .iter()
        .any(|d| !d.held.is_empty())
    {
        return Err(HarnessError::Contract);
    }
    for requirement in input.requirements().required() {
        match requirement.control {
            Control::StandardInput
            | Control::StructuredOutput
            | Control::Timeout
            | Control::Cancellation
            | Control::Resource(Resource::TimeMillis | Resource::OutputBytes | Resource::Workers) =>
                {}
            other => return Err(HarnessError::Unsupported(other)),
        }
    }
    let l = &spec.limits;
    if Instant::now()
        .checked_add(Duration::from_millis(l.wall_ms))
        .is_none()
        || Instant::now()
            .checked_add(Duration::from_millis(l.cleanup_ms))
            .is_none()
        || spec.containment.as_ref().is_some_and(|scope| {
            scope
                .elapsed_envelope_ms(l)
                .and_then(|ms| Instant::now().checked_add(Duration::from_millis(ms)))
                .is_none()
        })
        || spec.input_len != input.bytes().len() as u64
    {
        return Err(HarnessError::Limits);
    }
    let cwd = fs::symlink_metadata(&spec.cwd)?;
    if !cwd.is_dir() || fs::canonicalize(&spec.cwd)? != spec.cwd {
        return Err(HarnessError::TargetChanged);
    }
    Ok(PinnedFixture {
        spec: spec.clone(),
        descriptor: input.descriptor().clone(),
        requirements: input.requirements().clone(),
        input: input.bytes().to_vec(),
        executable: PinnedExecutable::inspect(&spec.executable, l.executable_bytes)?,
        cwd: stamp(&cwd),
        scope: Mutex::new(match &spec.containment {
            Some(_) => Some(PreparedScope::inspect(
                spec,
                deadline(Instant::now(), l.wall_ms)?,
            )?),
            None => None,
        }),
    })
}

struct ChildGuard {
    child: Child,
    cleanup: Duration,
    reaped: Option<ExitStatus>,
}
impl ChildGuard {
    fn poll(&mut self) -> io::Result<Option<ExitStatus>> {
        if self.reaped.is_none() {
            self.reaped = self.child.try_wait()?;
        }
        Ok(self.reaped)
    }
    fn stop(&mut self) {
        if self.reaped.is_none() {
            let _ = self.child.kill();
        }
    }
}
impl Drop for ChildGuard {
    fn drop(&mut self) {
        self.stop();
        let Some(deadline) = Instant::now().checked_add(self.cleanup) else {
            return;
        };
        while self.reaped.is_none() && Instant::now() < deadline {
            match self.poll() {
                Ok(None) => thread::sleep(POLL),
                _ => break,
            }
        }
    }
}
// Private observation boundary: production installs only PreparedScope. Tests
// may supply controlled facts without activating a host isolation backend.
trait ScopeRuntime {
    fn observe(
        &mut self,
        process: Option<&ProcessIdentity>,
        deadline: Instant,
    ) -> Result<ScopeObservation, HarnessError>;
    fn last_observation(&self) -> Option<ScopeObservation>;
    fn unresolved_helpers(&self) -> Vec<ScopeHelperIdentity>;
    fn terminate(&mut self, signal: ScopeSignal, deadline: Instant) -> Result<(), HarnessError>;
}
impl ScopeRuntime for PreparedScope {
    fn observe(
        &mut self,
        process: Option<&ProcessIdentity>,
        deadline: Instant,
    ) -> Result<ScopeObservation, HarnessError> {
        PreparedScope::observe(self, process, deadline)
    }
    fn last_observation(&self) -> Option<ScopeObservation> {
        PreparedScope::last_observation(self)
    }
    fn unresolved_helpers(&self) -> Vec<ScopeHelperIdentity> {
        PreparedScope::unresolved_helpers(self)
    }
    fn terminate(&mut self, signal: ScopeSignal, deadline: Instant) -> Result<(), HarnessError> {
        PreparedScope::terminate(self, signal, deadline)
    }
}
struct Running {
    child: ChildGuard,
    stdin: UnixStream,
    stdout: UnixStream,
    stderr: UnixStream,
    identity: Option<ProcessIdentity>,
    start: Instant,
    scope: Option<Box<dyn ScopeRuntime>>,
}
fn start_ticks(pid: u32) -> io::Result<u64> {
    let mut data = Vec::new();
    File::open(format!("/proc/{pid}/stat"))?
        .take(8193)
        .read_to_end(&mut data)?;
    if data.len() > 8192 {
        return Err(io::ErrorKind::InvalidData.into());
    }
    let end = data
        .windows(2)
        .rposition(|p| p == b") ")
        .ok_or(io::ErrorKind::InvalidData)?;
    std::str::from_utf8(&data[end + 2..])
        .map_err(|_| io::ErrorKind::InvalidData)?
        .split_whitespace()
        .nth(19)
        .ok_or(io::ErrorKind::InvalidData)?
        .parse::<u64>()
        .map_err(|_| io::ErrorKind::InvalidData.into())
}
fn pipe() -> io::Result<(UnixStream, Stdio)> {
    let (parent, child) = UnixStream::pair()?;
    parent.set_nonblocking(true)?;
    Ok((parent, Stdio::from(OwnedFd::from(child))))
}
fn deadline(start: Instant, ms: u64) -> Result<Instant, HarnessError> {
    start
        .checked_add(Duration::from_millis(ms))
        .ok_or(HarnessError::Limits)
}
struct SpawnFailure {
    error: HarnessError,
    scope: Option<Box<PreparedScope>>,
}
impl fmt::Debug for SpawnFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error.fmt(f)
    }
}
#[cfg(test)]
fn spawn(prepared: &PinnedFixture, start: Instant) -> Result<Running, SpawnFailure> {
    spawn_checked(prepared, start, || true)
}
fn spawn_checked(
    prepared: &PinnedFixture,
    start: Instant,
    mut current: impl FnMut() -> bool,
) -> Result<Running, SpawnFailure> {
    let mut scope = prepared
        .scope
        .lock()
        .map_err(|_| SpawnFailure {
            error: HarnessError::Contract,
            scope: None,
        })?
        .take();
    let result = (|| -> Result<_, HarnessError> {
        if prepared.spec.containment.is_some() != scope.is_some() {
            return Err(HarnessError::Contract);
        }
        let (stdin, child_in) = pipe()?;
        let (stdout, child_out) = pipe()?;
        let (stderr, child_err) = pipe()?;
        let mut command = if let Some(scope) = &mut scope {
            let ms = prepared
                .spec
                .limits
                .transport
                .as_ref()
                .map_or(prepared.spec.limits.wall_ms, |t| {
                    t.startup_ms.min(prepared.spec.limits.wall_ms)
                });
            // This callback is reached only after durable dispatch uncertainty.
            // Helper failure remains unknown; it can never certify no effect.
            scope.prepare_dispatch(deadline(start, ms)?)?;
            scope.command(&prepared.spec)?
        } else {
            // The open ELF pin covers this first exec only. Wrapper chains use
            // explicit helper/file pins under the cooperating-owner contract.
            let mut command = Command::new(format!(
                "/proc/self/fd/{}",
                prepared.executable.file.as_raw_fd()
            ));
            command
                .args(&prepared.spec.args)
                .env_clear()
                .current_dir(&prepared.spec.cwd);
            command
        };
        // Helpers and file pins may consume time. Current grant validity and
        // cancellation must be checked again at the actual mutation boundary.
        prepared.executable.check()?;
        if !current() {
            return Err(HarnessError::Contract);
        }
        let child = command
            .stdin(child_in)
            .stdout(child_out)
            .stderr(child_err)
            .spawn()?;
        // Command retains configured Stdio handles; closing this parent copy is
        // necessary before EOF can mean all inherited child writers closed.
        drop(command);
        Ok((child, stdin, stdout, stderr))
    })();
    let (child, stdin, stdout, stderr) = result.map_err(|error| SpawnFailure {
        error,
        scope: scope.take().map(Box::new),
    })?;
    let identity = start_ticks(child.id())
        .ok()
        .filter(|v| *v > 0)
        .map(|ticks| ProcessIdentity {
            launch: prepared.spec.reference.clone(),
            pid: child.id(),
            start_ticks: ticks,
        });
    Ok(Running {
        child: ChildGuard {
            child,
            cleanup: Duration::from_millis(prepared.spec.limits.cleanup_ms),
            reaped: None,
        },
        stdin,
        stdout,
        stderr,
        identity,
        start,
        scope: scope.map(|scope| Box::new(scope) as Box<dyn ScopeRuntime>),
    })
}
fn elapsed(start: Instant) -> u64 {
    u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX)
}
fn summary(spec: &ProcessSpec, id: ObservationId, basis: SourceReference) -> ProcessObservation {
    ProcessObservation {
        id,
        spec: spec.reference.clone(),
        process: None,
        completion: ProcessCompletion::Unknown,
        quiescence: Quiescence::Unknown,
        input_bytes: 0,
        stdout_bytes: 0,
        stderr_bytes: 0,
        structured_records: 0,
        malformed_output: false,
        elapsed_ms: 0,
        basis,
        containment: None,
        unresolved_scope_helpers: vec![],
    }
}
struct Framing {
    bytes: Vec<u8>,
    records: u64,
    malformed: bool,
    record_limit: Option<u64>,
}
impl Framing {
    fn valid(&self) -> bool {
        if self.record_limit.is_some() {
            // The extended transport validates JSON syntax without constructing
            // a semantic tree. Historical fixtures keep their original parser.
            serde_json::from_slice::<&serde_json::value::RawValue>(&self.bytes).is_ok()
        } else {
            serde_json::from_slice::<serde_json::Value>(&self.bytes).is_ok()
        }
    }
    fn record(&mut self) -> Result<(), InterruptionReason> {
        self.records = self
            .records
            .checked_add(1)
            .ok_or(InterruptionReason::EventLimit)?;
        if self.record_limit.is_some_and(|max| self.records > max) {
            return Err(InterruptionReason::EventLimit);
        }
        Ok(())
    }
    fn finish(&mut self) -> Result<(), InterruptionReason> {
        if !self.bytes.is_empty() {
            if !self.valid() {
                self.malformed = true;
                return Err(InterruptionReason::MalformedOutput);
            }
            self.record()?;
            self.bytes.clear();
        }
        Ok(())
    }
    fn feed(&mut self, input: &[u8], max: u64) -> Result<(), InterruptionReason> {
        for byte in input {
            if *byte == b'\n' {
                if self.bytes.is_empty() || !self.valid() {
                    self.malformed = true;
                    return Err(InterruptionReason::MalformedOutput);
                }
                self.record()?;
                self.bytes.clear();
            } else {
                if self.bytes.len() as u64 >= max {
                    self.malformed = true;
                    return Err(InterruptionReason::MalformedOutput);
                }
                self.bytes.push(*byte);
            }
        }
        Ok(())
    }
}
fn capture(
    stream: &mut UnixStream,
    count: &mut u64,
    other: u64,
    limit: u64,
    framing: Option<&mut Framing>,
    frame: u64,
    stream_limit: Option<(u64, InterruptionReason)>,
) -> Result<bool, InterruptionReason> {
    let used = count
        .checked_add(other)
        .ok_or(InterruptionReason::OutputLimit)?;
    let remaining = limit.saturating_sub(used).saturating_add(1).min(
        stream_limit.map_or(u64::MAX, |(max, _)| {
            max.saturating_sub(*count).saturating_add(1)
        }),
    );
    let size = usize::try_from(remaining.min(8192)).map_err(|_| InterruptionReason::OutputLimit)?;
    let mut bytes = [0_u8; 8192];
    match stream.read(&mut bytes[..size]) {
        Ok(0) => Ok(true),
        Ok(n) => {
            *count = count
                .checked_add(n as u64)
                .ok_or(InterruptionReason::OutputLimit)?;
            if count.checked_add(other).is_none_or(|n| n > limit) {
                return Err(InterruptionReason::OutputLimit);
            }
            if let Some((max, cause)) = stream_limit
                && *count > max
            {
                return Err(cause);
            }
            if let Some(framing) = framing {
                framing.feed(&bytes[..n], frame)?;
            }
            Ok(false)
        }
        Err(e)
            if matches!(
                e.kind(),
                io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
            ) =>
        {
            Ok(false)
        }
        Err(_) => Err(InterruptionReason::Io),
    }
}
#[derive(Default)]
struct Cleanup {
    end: Option<Instant>,
    phases: Option<(Instant, Instant)>,
    killed: bool,
}
impl Cleanup {
    fn query_deadline(&self, now: Instant, wall_end: Instant) -> Instant {
        match self.phases {
            Some((term, _)) if now < term => term,
            Some((_, kill)) if now < kill => kill,
            _ => self.end.unwrap_or(wall_end),
        }
    }
    fn advance(
        &mut self,
        running: &mut Running,
        spec: &ProcessSpec,
        result: &mut ProcessObservation,
    ) {
        if self.end.is_none() {
            let _ = running.stdin.shutdown(Shutdown::Both);
            let now = Instant::now();
            if let (Some(scope), Some(contract)) = (&mut running.scope, &spec.containment) {
                let maximum = contract
                    .elapsed_envelope_ms(&spec.limits)
                    .and_then(|ms| deadline(running.start, ms).ok())
                    .unwrap_or(now);
                let term = deadline(now, contract.cancellation_grace_ms)
                    .unwrap_or(now)
                    .min(maximum);
                let kill = deadline(term, contract.escalation_ms)
                    .unwrap_or(term)
                    .min(maximum);
                self.end = Some(
                    deadline(kill, spec.limits.cleanup_ms)
                        .unwrap_or(kill)
                        .min(maximum),
                );
                self.phases = Some((term, kill));
                let _ = scope.terminate(ScopeSignal::Terminate, term);
                result.containment = scope.last_observation().map(Box::new);
            } else {
                running.child.stop();
                self.end = Some(deadline(now, spec.limits.cleanup_ms).unwrap_or(now));
            }
        }
        if let Some((term, kill)) = self.phases
            && !self.killed
            && Instant::now() >= term
        {
            if let Some(scope) = &mut running.scope {
                let _ = scope.terminate(ScopeSignal::Kill, kill);
                result.containment = scope.last_observation().map(Box::new);
            }
            // Only our retained wrapper is stopped here. This cannot establish
            // quiescence for an unknown scope or surviving descendants.
            running.child.stop();
            self.killed = true;
        }
    }
}
#[cfg(test)]
fn supervise(
    running: Running,
    prepared: &PinnedFixture,
    ids: &CaptureIds,
    cancel: &AtomicBool,
    authority: impl FnMut(u64) -> Result<(), InterruptionReason>,
    initial_reason: Option<InterruptionReason>,
) -> ProcessObservation {
    supervise_with_checkpoint(
        running,
        prepared,
        ids,
        cancel,
        authority,
        initial_reason,
        |_| Err(InterruptionReason::AuthorityUnknown),
    )
}
fn supervise_with_checkpoint(
    mut running: Running,
    prepared: &PinnedFixture,
    ids: &CaptureIds,
    cancel: &AtomicBool,
    mut authority: impl FnMut(u64) -> Result<(), InterruptionReason>,
    initial_reason: Option<InterruptionReason>,
    mut publish_identity: impl FnMut(&ProcessObservation) -> Result<(), InterruptionReason>,
) -> ProcessObservation {
    let spec = &prepared.spec;
    let limits = &spec.limits;
    let mut result = summary(spec, ids.finished_observation.clone(), ids.basis.clone());
    result.process = running.identity.clone();
    let mut reason =
        initial_reason.or_else(|| running.identity.is_none().then_some(InterruptionReason::Io));
    let mut framing = Framing {
        bytes: vec![],
        records: 0,
        malformed: false,
        record_limit: limits.transport.as_ref().map(|t| t.records),
    };
    let mut buffers_usable = true;
    if let Some(transport) = &limits.transport {
        let reserve = usize::try_from(limits.frame_bytes)
            .ok()
            .and_then(|n| framing.bytes.try_reserve_exact(n).ok());
        let buffers = (prepared.input.capacity() as u64)
            .checked_add(framing.bytes.capacity() as u64)
            .and_then(|n| n.checked_add(8192));
        if reserve.is_none() || buffers.is_none_or(|n| n > transport.buffer_bytes) {
            buffers_usable = false;
            reason.get_or_insert(InterruptionReason::BufferLimit);
        }
    }
    let mut out_eof = false;
    let mut err_eof = false;
    // If the selected buffer allocation could not be honored, do not enter a
    // capture path that could grow an unreserved frame buffer during cleanup.
    let mut out_failed = !buffers_usable;
    let mut err_failed = out_failed;
    let mut in_closed = false;
    let mut next_authority = 0_u64;
    let mut cleanup = Cleanup::default();
    let mut last_progress = 0_u64;
    let mut first_record = None;
    let mut scope_ready = running.scope.is_none();
    let mut identity_published = spec.identity_checkpoint.is_none();
    let mut next_scope = 0_u64;
    let wall_end = deadline(running.start, limits.wall_ms).expect("validated wall deadline");
    loop {
        let ms = elapsed(running.start);
        let transport_deadline = |ms: u64, first_record: Option<u64>, last_progress: u64| {
            limits.transport.as_ref().and_then(|t| {
                if first_record.is_none_or(|first| first >= t.startup_ms) && ms >= t.startup_ms {
                    Some(InterruptionReason::StartupTimeout)
                } else if ms.saturating_sub(last_progress) >= t.idle_ms {
                    Some(InterruptionReason::IdleTimeout)
                } else {
                    None
                }
            })
        };
        if reason.is_none() {
            if cancel.load(Ordering::Acquire) {
                reason = Some(InterruptionReason::Cancelled);
            } else if ms >= limits.wall_ms {
                reason = Some(InterruptionReason::Timeout);
            } else if let Some(cause) = transport_deadline(ms, first_record, last_progress) {
                reason = Some(cause);
            } else if ms >= next_authority {
                if let Err(e) = authority(ms) {
                    reason = Some(e);
                }
                next_authority = elapsed(running.start).saturating_add(limits.authority_poll_ms);
            }
        }
        if reason.is_none() && elapsed(running.start) >= limits.wall_ms {
            reason = Some(InterruptionReason::Timeout);
        }
        if reason.is_none() {
            reason = transport_deadline(elapsed(running.start), first_record, last_progress);
        }
        // Escalation always precedes another manager readback. Every query is
        // clamped to the next phase boundary instead of borrowing cleanup time.
        if reason.is_some() {
            cleanup.advance(&mut running, spec, &mut result);
        }
        if let Some(scope) = &mut running.scope
            && ms >= next_scope
        {
            let end = cleanup.query_deadline(Instant::now(), wall_end);
            let needs_membership = scope
                .last_observation()
                .is_none_or(|o| !o.membership_verified);
            match scope.observe(
                if needs_membership {
                    running.identity.as_ref()
                } else {
                    None
                },
                end,
            ) {
                Ok(observed) => {
                    scope_ready = spec
                        .containment
                        .as_ref()
                        .is_some_and(|expected| observed.corresponds(expected));
                    if !scope_ready {
                        reason.get_or_insert(InterruptionReason::ContainmentUnknown);
                    }
                    if let Some(expected) = &spec.containment
                        && observed
                            .cpu_usage_us
                            .is_some_and(|cpu| cpu >= expected.cpu_time_ms.saturating_mul(1000))
                    {
                        reason.get_or_insert(InterruptionReason::CpuTimeLimit);
                    }
                    result.containment = Some(Box::new(observed));
                }
                Err(_) => {
                    result.containment = scope.last_observation().map(Box::new);
                    // A scope may not yet exist while its one committed launch is
                    // starting. No input is released during this bounded wait.
                    if scope_ready || result.containment.is_some() || cleanup.end.is_some() {
                        reason.get_or_insert(InterruptionReason::ContainmentUnknown);
                    }
                    scope_ready = false;
                }
            }
            next_scope = elapsed(running.start).saturating_add(limits.authority_poll_ms);
        }
        // A bounded helper can consume the remaining time; recheck before I/O.
        if reason.is_none() {
            let now_ms = elapsed(running.start);
            reason = if now_ms >= limits.wall_ms {
                Some(InterruptionReason::Timeout)
            } else {
                transport_deadline(now_ms, first_record, last_progress)
            };
        }
        if reason.is_none() && scope_ready && !identity_published {
            // Acquisition itself may block while cancellation or current grant
            // state changes. Recheck the publication boundary independently of
            // the later input boundary; a checkpoint is not a harmless retry.
            if cancel.load(Ordering::Acquire) {
                reason = Some(InterruptionReason::Cancelled);
            } else {
                reason = authority(elapsed(running.start)).err();
            }
            if reason.is_none() {
                let now = elapsed(running.start);
                reason = if cancel.load(Ordering::Acquire) {
                    Some(InterruptionReason::Cancelled)
                } else if now >= limits.wall_ms {
                    Some(InterruptionReason::Timeout)
                } else {
                    transport_deadline(now, first_record, last_progress)
                };
            }
        }
        if reason.is_none() && scope_ready && !identity_published {
            let mut checkpoint = result.clone();
            checkpoint.completion = ProcessCompletion::IdentityVerified;
            checkpoint.elapsed_ms = elapsed(running.start);
            checkpoint.structured_records = framing.records;
            checkpoint.malformed_output = framing.malformed;
            checkpoint.unresolved_scope_helpers = running
                .scope
                .as_ref()
                .map_or_else(Vec::new, |scope| scope.unresolved_helpers());
            if checkpoint.validate(spec).is_err() {
                reason = Some(InterruptionReason::ContainmentUnknown);
            } else if let Err(cause) = publish_identity(&checkpoint) {
                reason = Some(cause);
            } else {
                identity_published = true;
                // Publication never refreshes the launch clock or authority.
                // A successful durable receipt alone cannot permit late input.
                let now = elapsed(running.start);
                reason = if cancel.load(Ordering::Acquire) {
                    Some(InterruptionReason::Cancelled)
                } else if now >= limits.wall_ms {
                    Some(InterruptionReason::Timeout)
                } else {
                    transport_deadline(now, first_record, last_progress)
                };
                if reason.is_none() {
                    reason = authority(now).err();
                }
                let now = elapsed(running.start);
                if reason.is_none() && now >= limits.wall_ms {
                    reason = Some(InterruptionReason::Timeout);
                }
                if reason.is_none() {
                    reason = transport_deadline(now, first_record, last_progress);
                }
                if reason.is_none() && cancel.load(Ordering::Acquire) {
                    reason = Some(InterruptionReason::Cancelled);
                }
            }
        }
        let previous_counts = (result.input_bytes, result.stdout_bytes, result.stderr_bytes);
        if reason.is_none() && scope_ready && identity_published && !in_closed {
            match running
                .stdin
                .write(&prepared.input[result.input_bytes as usize..])
            {
                Ok(0) if result.input_bytes < prepared.input.len() as u64 => {
                    reason = Some(InterruptionReason::InputClosed)
                }
                Ok(n) => result.input_bytes += n as u64,
                Err(e)
                    if matches!(
                        e.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                    ) => {}
                Err(e) if e.kind() == io::ErrorKind::BrokenPipe => {
                    reason = Some(InterruptionReason::InputClosed)
                }
                Err(_) => reason = Some(InterruptionReason::Io),
            }
            if result.input_bytes == prepared.input.len() as u64 {
                let _ = running.stdin.shutdown(Shutdown::Write);
                in_closed = true;
            }
        }
        if !out_eof && !out_failed {
            match capture(
                &mut running.stdout,
                &mut result.stdout_bytes,
                result.stderr_bytes,
                limits.output_bytes,
                Some(&mut framing),
                limits.frame_bytes,
                limits
                    .transport
                    .as_ref()
                    .map(|t| (t.stdout_bytes, InterruptionReason::StdoutLimit)),
            ) {
                Ok(v) => out_eof = v,
                Err(e) => {
                    reason.get_or_insert(e);
                    out_failed = true;
                }
            }
        }
        if !err_eof && !err_failed {
            match capture(
                &mut running.stderr,
                &mut result.stderr_bytes,
                result.stdout_bytes,
                limits.output_bytes,
                None,
                limits.frame_bytes,
                limits
                    .transport
                    .as_ref()
                    .map(|t| (t.stderr_bytes, InterruptionReason::StderrLimit)),
            ) {
                Ok(v) => err_eof = v,
                Err(e) => {
                    reason.get_or_insert(e);
                    err_failed = true;
                }
            }
        }
        if out_eof && let Err(error) = framing.finish() {
            reason.get_or_insert(error);
            framing.bytes.clear();
        }
        let observed_ms = elapsed(running.start);
        if first_record.is_none() && framing.records > 0 {
            first_record = Some(observed_ms);
        }
        if reason.is_none() {
            reason = if observed_ms >= limits.wall_ms {
                Some(InterruptionReason::Timeout)
            } else {
                transport_deadline(observed_ms, first_record, last_progress)
            };
        }
        if previous_counts != (result.input_bytes, result.stdout_bytes, result.stderr_bytes) {
            last_progress = observed_ms;
        }
        if running.child.poll().is_err() {
            reason.get_or_insert(InterruptionReason::Io);
        }
        if running.child.reaped.is_some() && out_eof && err_eof {
            if result.input_bytes < spec.input_len {
                reason.get_or_insert(InterruptionReason::InputClosed);
            }
            if limits.transport.is_some() && framing.records == 0 {
                reason.get_or_insert(InterruptionReason::StartupIncomplete);
            }
            if reason.is_none()
                && let Err(e) = authority(elapsed(running.start))
            {
                reason = Some(e);
            }
            let whole_set_empty = match &spec.containment {
                None => running.identity.is_some(),
                Some(expected) => result
                    .containment
                    .as_ref()
                    .is_some_and(|o| o.corresponds(expected) && o.populated == Some(false)),
            };
            if whole_set_empty {
                result.quiescence = if spec.containment.is_some() {
                    Quiescence::OwnedScopeEmpty
                } else {
                    Quiescence::DirectChildReapedStreamsClosed
                };
                break;
            }
            reason.get_or_insert(InterruptionReason::ContainmentUnknown);
        }
        if reason.is_some() {
            cleanup.advance(&mut running, spec, &mut result);
        }
        if cleanup.end.is_some_and(|end| Instant::now() >= end) {
            break;
        }
        thread::sleep(POLL);
    }
    if let Some(scope) = &running.scope {
        result.unresolved_scope_helpers = scope.unresolved_helpers();
    }
    result.elapsed_ms = elapsed(running.start);
    result.structured_records = framing.records;
    result.malformed_output = framing.malformed;
    result.completion = if let Some(reason) = reason {
        ProcessCompletion::Interrupted { reason }
    } else if let Some(status) = running.child.reaped {
        ProcessCompletion::Exited {
            code: status.code(),
            signal: status.signal(),
        }
    } else {
        ProcessCompletion::Unknown
    };
    running.child.cleanup = Duration::ZERO;
    result
}

struct ReceiptWriter<'a> {
    store: &'a ProgramStore,
    key: &'a StoreKey,
    dispatch: &'a Operation,
    effect: &'a EffectId,
    scope: &'a EffectScope,
    attempts: usize,
    stale: Vec<Operation>,
    reconciled: Vec<ReconciledReceipt>,
}
impl ReceiptWriter<'_> {
    fn record(
        &mut self,
        id: &OperationId,
        observation: &ProcessObservation,
        tick: u64,
    ) -> Result<Receipt, AdmissionFailure> {
        let Self {
            store,
            key,
            dispatch,
            effect,
            scope,
            attempts,
            stale,
            reconciled,
        } = self;
        for _ in 0..*attempts {
            let kernel = store
                .load(key, Access::ReadOnly)
                .map_err(|error| AdmissionFailure {
                    completion: Completion::Rejected,
                    error,
                })?;
            let input = Operation {
                id: id.clone(),
                expected: kernel.revision(),
                grant: dispatch.grant.clone(),
                command: WorkflowCommand::SupervisedEffect(Box::new(
                    EffectEvent::ProcessObserved {
                        effect: (*effect).clone(),
                        scope: (*scope).clone(),
                        at: tick.max(kernel.fixture_effects().last_tick()),
                        observation: observation.clone(),
                    },
                )),
            };
            match store.admit(key, input.clone()) {
                Ok(receipt) => return Ok(receipt),
                Err(failure) => {
                    if failure.completion == Completion::Indeterminate {
                        if observation.completion == ProcessCompletion::IdentityVerified {
                            return Err(failure);
                        }
                        if let Ok(Reconciliation::Present(receipt)) =
                            store.reconcile(key, kernel.initial(), &input, Access::Recover)
                        {
                            reconciled.push(ReconciledReceipt {
                                operation: input,
                                failure,
                                receipt,
                            });
                            return Ok(receipt);
                        }
                        return Err(failure);
                    }
                    let retry = failure.completion == Completion::Rejected
                        && matches!(
                            &failure.error,
                            ProgramStoreError::Domain(cortex_evidence::program::KernelError::Core(
                                ProgramError::Stale
                            ))
                        );
                    if retry {
                        stale.push(input);
                    } else {
                        return Err(failure);
                    }
                }
            }
        }
        Err(AdmissionFailure {
            completion: Completion::Rejected,
            error: ProgramStoreError::Domain(ProgramError::Stale.into()),
        })
    }
}

fn launch_after_commit(
    prepared: &PinnedFixture,
    current: &CoordinationKernel,
    effect: &EffectId,
    scope: &EffectScope,
    at: u64,
    start: Instant,
    cancel: &AtomicBool,
) -> Result<Option<Running>, SpawnFailure> {
    if cancel.load(Ordering::Acquire) || prepared.check(current, effect).is_err() {
        return Ok(None);
    }
    let Some(tick) = at.checked_add(elapsed(start)) else {
        return Ok(None);
    };
    if current
        .fixture_effects()
        .check_supervised_current(current.state(), effect, scope, tick)
        .is_err()
        || elapsed(start) >= prepared.spec.limits.wall_ms
        || prepared
            .spec
            .limits
            .transport
            .as_ref()
            .is_some_and(|t| elapsed(start) >= t.startup_ms)
        || cancel.load(Ordering::Acquire)
    {
        // Every branch precedes spawn, so absence is observed rather than inferred
        // from a timeout or an ambiguous operating-system spawn error.
        return Ok(None);
    }
    spawn_checked(prepared, start, || {
        let ms = elapsed(start);
        !cancel.load(Ordering::Acquire)
            && ms < prepared.spec.limits.wall_ms
            && prepared
                .spec
                .limits
                .transport
                .as_ref()
                .is_none_or(|t| ms < t.startup_ms)
            && at.checked_add(ms).is_some_and(|tick| {
                current
                    .fixture_effects()
                    .check_supervised_current(current.state(), effect, scope, tick)
                    .is_ok()
            })
    })
    .map(Some)
}

/// Run one explicitly retained trusted fixture through the canonical store gate.
/// The dispatch must be a new `SupervisedEffect::Dispatch` in format 3. The store
/// commits uncertainty before the launch callback; duplicates and uncertain
/// commits never invoke it. Startup identity and final summary are appended under
/// their exact supplied IDs. A missing final receipt remains unresolved.
///
/// This supports cooperating local operators and an unchanged owned ELF fixture
/// on a responsive Linux filesystem. Ambient environment is cleared; the narrow
/// scope-only profile adds explicit manager routing metadata, and rootless
/// fixtures clear it again. No shell or PATH resolution is used. Exact byte pinning is not authentication or hostile-write
/// protection. Canonical grant/cancel state is observed cooperatively; store and
/// kernel calls can delay polling. The optional owned-scope primitive retains
/// observed whole-set identity and controls; its configured profile is not host
/// qualification. It provides no instantaneous revocation or hard cumulative CPU
/// guarantee. Required unsupported controls remain refused. Raw output is bounded, validated where applicable and
/// discarded; only counts and typed causes are retained. No PID is reattached.
pub fn execute_fixture(
    store: &ProgramStore,
    key: &StoreKey,
    dispatch: Operation,
    ids: CaptureIds,
    prepared: &PinnedFixture,
    cancel: &AtomicBool,
) -> Result<RunResult, HarnessError> {
    execute_fixture_inner(store, key, dispatch, ids, None, prepared, cancel)
}

/// Execute the explicitly selected identity-checkpoint contract. The new IDs
/// identify a canonical observation committed before any package byte is sent.
/// Refused or uncertain publication withholds input; replay never launches.
pub fn execute_fixture_with_identity_checkpoint(
    store: &ProgramStore,
    key: &StoreKey,
    dispatch: Operation,
    ids: CaptureIds,
    identity_ids: IdentityCaptureIds,
    prepared: &PinnedFixture,
    cancel: &AtomicBool,
) -> Result<RunResult, HarnessError> {
    execute_fixture_inner(
        store,
        key,
        dispatch,
        ids,
        Some(identity_ids),
        prepared,
        cancel,
    )
}

fn execute_fixture_inner(
    store: &ProgramStore,
    key: &StoreKey,
    dispatch: Operation,
    ids: CaptureIds,
    identity_ids: Option<IdentityCaptureIds>,
    prepared: &PinnedFixture,
    cancel: &AtomicBool,
) -> Result<RunResult, HarnessError> {
    if prepared.spec.identity_checkpoint.is_some() != identity_ids.is_some() {
        return Err(HarnessError::Contract);
    }
    let (effect, scope, at) = match &dispatch.command {
        WorkflowCommand::SupervisedEffect(event) => match &**event {
            EffectEvent::Dispatch {
                effect, scope, at, ..
            } => (effect.clone(), scope.clone(), *at),
            _ => return Err(HarnessError::Contract),
        },
        _ => return Err(HarnessError::Contract),
    };
    let kernel = store
        .load(key, Access::ReadOnly)
        .map_err(HarnessError::Store)?;
    if ids.admission_attempts == 0
        || ids.admission_attempts > kernel.state().definition().limits().entries
        || ids.started_operation == ids.finished_operation
        || ids.started_operation == dispatch.id
        || ids.finished_operation == dispatch.id
        || ids.started_observation == ids.finished_observation
        || identity_ids.as_ref().is_some_and(|identity| {
            [
                &ids.started_operation,
                &ids.finished_operation,
                &dispatch.id,
            ]
            .contains(&&identity.checkpoint_operation)
                || [&ids.started_observation, &ids.finished_observation]
                    .contains(&&identity.checkpoint_observation)
        })
    {
        return Err(HarnessError::Contract);
    }
    // Historical duplicate reconciliation does not depend on a still-existing
    // executable or an otherwise-current package; it can never relaunch.
    if kernel.operations().iter().any(|o| o.id == dispatch.id) {
        let (receipt, _) = store
            .dispatch_fixture(key, dispatch, |_| ())
            .map_err(HarnessError::Admission)?;
        return Ok(RunResult {
            dispatch: receipt,
            started: None,
            identity_checkpoint: None,
            observation: None,
            recorded: None,
            recording_failure: None,
            stale_receipt_attempts: vec![],
            reconciled_receipt_failures: vec![],
        });
    }
    if kernel.operations().iter().any(|operation| {
        operation.id == ids.started_operation
            || operation.id == ids.finished_operation
            || identity_ids
                .as_ref()
                .is_some_and(|i| operation.id == i.checkpoint_operation)
    }) || kernel
        .fixture_effects()
        .effects()
        .iter()
        .flat_map(|effect| &effect.process_observations)
        .any(|observation| {
            observation.id == ids.started_observation
                || observation.id == ids.finished_observation
                || identity_ids
                    .as_ref()
                    .is_some_and(|i| observation.id == i.checkpoint_observation)
        })
    {
        return Err(HarnessError::Contract);
    }
    prepared.check(&kernel, &effect)?;
    if store.read_limits().timeout()
        > Duration::from_millis(
            prepared
                .spec
                .limits
                .wall_ms
                .min(prepared.spec.limits.cleanup_ms),
        )
    {
        return Err(HarnessError::Limits);
    }
    let start = Instant::now();
    let (receipt, launched) = store
        .dispatch_fixture(key, dispatch.clone(), |current| {
            launch_after_commit(prepared, current, &effect, &scope, at, start, cancel)
        })
        .map_err(HarnessError::Admission)?;
    let Some(launched) = launched else {
        return Ok(RunResult {
            dispatch: receipt,
            started: None,
            identity_checkpoint: None,
            observation: None,
            recorded: None,
            recording_failure: None,
            stale_receipt_attempts: vec![],
            reconciled_receipt_failures: vec![],
        });
    };
    let mut writer = ReceiptWriter {
        store,
        key,
        dispatch: &dispatch,
        effect: &effect,
        scope: &scope,
        attempts: ids.admission_attempts,
        stale: vec![],
        reconciled: vec![],
    };
    let mut started = None;
    let mut identity_checkpoint = None;
    let mut recording_failure = None;
    let observation = match launched {
        Ok(Some(running)) => {
            let mut observation = summary(
                &prepared.spec,
                ids.started_observation.clone(),
                ids.basis.clone(),
            );
            observation.process = running.identity.clone();
            observation.elapsed_ms = elapsed(running.start);
            observation.completion = if observation.process.is_some() {
                ProcessCompletion::Spawned
            } else {
                ProcessCompletion::Unknown
            };
            match writer.record(
                &ids.started_operation,
                &observation,
                at.saturating_add(observation.elapsed_ms),
            ) {
                Ok(r) => started = Some(r),
                Err(e) => recording_failure = Some(e),
            }
            let initial = recording_failure
                .as_ref()
                .map(|_| InterruptionReason::AuthorityUnknown);
            supervise_with_checkpoint(
                running,
                prepared,
                &ids,
                cancel,
                |_ms| {
                    let kernel = store
                        .load(key, Access::ReadOnly)
                        .map_err(|_| InterruptionReason::AuthorityUnknown)?;
                    let tick = at
                        .checked_add(elapsed(start))
                        .ok_or(InterruptionReason::AuthorityUnknown)?
                        .max(kernel.fixture_effects().last_tick());
                    kernel
                        .fixture_effects()
                        .check_supervised_current(kernel.state(), &effect, &scope, tick)
                        .map_err(|_| InterruptionReason::AuthorityLost)
                },
                initial,
                |captured| {
                    let identity_ids = identity_ids
                        .as_ref()
                        .ok_or(InterruptionReason::AuthorityUnknown)?;
                    let mut observation = captured.clone();
                    observation.id = identity_ids.checkpoint_observation.clone();
                    match writer.record(
                        &identity_ids.checkpoint_operation,
                        &observation,
                        at.saturating_add(elapsed(start)),
                    ) {
                        Ok(receipt) => {
                            identity_checkpoint = Some(receipt);
                            Ok(())
                        }
                        Err(failure) => {
                            recording_failure = Some(failure);
                            Err(InterruptionReason::AuthorityUnknown)
                        }
                    }
                },
            )
        }
        Ok(None) => {
            let mut observation = summary(
                &prepared.spec,
                ids.finished_observation.clone(),
                ids.basis.clone(),
            );
            observation.completion = ProcessCompletion::StartFailed;
            observation.quiescence = Quiescence::NoChildStarted;
            observation.elapsed_ms = elapsed(start);
            observation
        }
        Err(mut failed) => {
            // A generic spawn or manager-helper error never establishes absence of external effects.
            let mut observation = summary(
                &prepared.spec,
                ids.finished_observation.clone(),
                ids.basis.clone(),
            );
            if let Some(scope) = &mut failed.scope {
                let envelope = prepared
                    .spec
                    .containment
                    .as_ref()
                    .and_then(|s| s.elapsed_envelope_ms(&prepared.spec.limits))
                    .unwrap_or(prepared.spec.limits.wall_ms);
                if let Ok(end) = deadline(start, envelope) {
                    let _ = scope.observe(None, end);
                }
                observation.containment = scope.last_observation().map(Box::new);
                observation.unresolved_scope_helpers = scope.unresolved_helpers();
            }
            observation.elapsed_ms = elapsed(start);
            observation
        }
    };
    let recorded = if recording_failure.is_none() {
        match writer.record(
            &ids.finished_operation,
            &observation,
            at.saturating_add(observation.elapsed_ms),
        ) {
            Ok(r) => Some(r),
            Err(e) => {
                recording_failure = Some(e);
                None
            }
        }
    } else {
        None
    };
    // The caller receives the final observation even when its append failed. The
    // canonical dispatch remains uncertain and cannot be retried as a new launch.
    Ok(RunResult {
        dispatch: receipt,
        started,
        identity_checkpoint,
        observation: Some(observation),
        recorded,
        recording_failure,
        stale_receipt_attempts: writer.stale,
        reconciled_receipt_failures: writer.reconciled,
    })
}

#[cfg(test)]
#[path = "supervisor_tests.rs"]
pub(crate) mod tests;
