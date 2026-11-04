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
    sync::atomic::{AtomicBool, Ordering},
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

/// An executed or replayed dispatch and its retained observations. No result here
/// reconciles resource reservations or releases an engineering dependency.
pub struct RunResult {
    /// Exact durable dispatch receipt; duplicates performed no launch.
    pub dispatch: Receipt,
    /// Positive spawn observation receipt, when durably retained.
    pub started: Option<Receipt>,
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
struct PinnedExecutable {
    path: PathBuf,
    file: File,
    stamp: Stamp,
    bytes: Vec<u8>,
}
impl PinnedExecutable {
    fn inspect(path: &Path, max: u64) -> Result<Self, HarnessError> {
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
    fn check(&self) -> Result<(), HarnessError> {
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
struct Running {
    child: ChildGuard,
    stdin: UnixStream,
    stdout: UnixStream,
    stderr: UnixStream,
    identity: Option<ProcessIdentity>,
    start: Instant,
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
fn spawn(prepared: &PinnedFixture, start: Instant) -> Result<Running, HarnessError> {
    let (stdin, child_in) = pipe()?;
    let (stdout, child_out) = pipe()?;
    let (stderr, child_err) = pipe()?;
    // Linux resolves this already-open ELF before close-on-exec. In-place writes
    // and loader/library replacement are outside the cooperating fixture profile.
    let child = Command::new(format!(
        "/proc/self/fd/{}",
        prepared.executable.file.as_raw_fd()
    ))
    .args(&prepared.spec.args)
    .env_clear()
    .current_dir(&prepared.spec.cwd)
    .stdin(child_in)
    .stdout(child_out)
    .stderr(child_err)
    .spawn()?;
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
    }
}
struct Framing {
    bytes: Vec<u8>,
    records: u64,
    malformed: bool,
}
impl Framing {
    fn finish(&mut self) -> Result<(), InterruptionReason> {
        if !self.bytes.is_empty() {
            if serde_json::from_slice::<serde_json::Value>(&self.bytes).is_err() {
                self.malformed = true;
                return Err(InterruptionReason::MalformedOutput);
            }
            self.records = self
                .records
                .checked_add(1)
                .ok_or(InterruptionReason::OutputLimit)?;
            self.bytes.clear();
        }
        Ok(())
    }
    fn feed(&mut self, input: &[u8], max: u64) -> Result<(), InterruptionReason> {
        for byte in input {
            if *byte == b'\n' {
                if self.bytes.is_empty()
                    || serde_json::from_slice::<serde_json::Value>(&self.bytes).is_err()
                {
                    self.malformed = true;
                    return Err(InterruptionReason::MalformedOutput);
                }
                self.records = self
                    .records
                    .checked_add(1)
                    .ok_or(InterruptionReason::OutputLimit)?;
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
) -> Result<bool, InterruptionReason> {
    let used = count
        .checked_add(other)
        .ok_or(InterruptionReason::OutputLimit)?;
    let size = usize::try_from(limit.saturating_sub(used).saturating_add(1).min(8192))
        .map_err(|_| InterruptionReason::OutputLimit)?;
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
fn supervise(
    mut running: Running,
    prepared: &PinnedFixture,
    ids: &CaptureIds,
    cancel: &AtomicBool,
    mut authority: impl FnMut(u64) -> Result<(), InterruptionReason>,
    initial_reason: Option<InterruptionReason>,
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
    };
    let mut out_eof = false;
    let mut err_eof = false;
    let mut out_failed = false;
    let mut err_failed = false;
    let mut in_closed = false;
    let mut next_authority = 0_u64;
    let mut cleanup_end = None;
    loop {
        let ms = elapsed(running.start);
        if reason.is_none() {
            if cancel.load(Ordering::Acquire) {
                reason = Some(InterruptionReason::Cancelled);
            } else if ms >= limits.wall_ms {
                reason = Some(InterruptionReason::Timeout);
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
        if reason.is_none() && !in_closed {
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
        if running.child.poll().is_err() {
            reason.get_or_insert(InterruptionReason::Io);
        }
        if reason.is_some() && cleanup_end.is_none() {
            running.child.stop();
            let _ = running.stdin.shutdown(Shutdown::Both);
            cleanup_end = Instant::now().checked_add(Duration::from_millis(limits.cleanup_ms));
        }
        if running.child.reaped.is_some() && out_eof && err_eof {
            if result.input_bytes < spec.input_len {
                reason.get_or_insert(InterruptionReason::InputClosed);
            }
            // Recheck canonical authority at the observed completion boundary.
            if reason.is_none()
                && let Err(e) = authority(elapsed(running.start))
            {
                reason = Some(e);
            }
            if running.identity.is_some() {
                result.quiescence = Quiescence::DirectChildReapedStreamsClosed;
            }
            break;
        }
        if cleanup_end.is_some_and(|end| Instant::now() >= end) {
            break;
        }
        thread::sleep(POLL);
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
) -> Result<Option<Running>, HarnessError> {
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
        || cancel.load(Ordering::Acquire)
    {
        // Every branch precedes spawn, so absence is observed rather than inferred
        // from a timeout or an ambiguous operating-system spawn error.
        return Ok(None);
    }
    spawn(prepared, start).map(Some)
}

/// Run one explicitly retained trusted fixture through the canonical store gate.
/// The dispatch must be a new `SupervisedEffect::Dispatch` in format 3. The store
/// commits uncertainty before the launch callback; duplicates and uncertain
/// commits never invoke it. Startup identity and final summary are appended under
/// their exact supplied IDs. A missing final receipt remains unresolved.
///
/// This supports cooperating local operators and an unchanged owned ELF fixture
/// on a responsive Linux filesystem. Environment is cleared; no shell or PATH
/// resolution is used. Exact byte pinning is not authentication or hostile-write
/// protection. Canonical grant/cancel state is observed cooperatively; store and
/// kernel calls can delay polling. No instantaneous revocation, process-tree,
/// filesystem, network, CPU or memory isolation is provided. Required unsupported
/// controls are refused. Raw output is bounded, validated where applicable and
/// discarded; only counts and typed causes are retained. No PID is reattached.
pub fn execute_fixture(
    store: &ProgramStore,
    key: &StoreKey,
    dispatch: Operation,
    ids: CaptureIds,
    prepared: &PinnedFixture,
    cancel: &AtomicBool,
) -> Result<RunResult, HarnessError> {
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
            observation: None,
            recorded: None,
            recording_failure: None,
            stale_receipt_attempts: vec![],
            reconciled_receipt_failures: vec![],
        });
    }
    if kernel.operations().iter().any(|operation| {
        operation.id == ids.started_operation || operation.id == ids.finished_operation
    }) || kernel
        .fixture_effects()
        .effects()
        .iter()
        .flat_map(|effect| &effect.process_observations)
        .any(|observation| {
            observation.id == ids.started_observation || observation.id == ids.finished_observation
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
            supervise(
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
        Err(_) => {
            // A generic spawn error never establishes absence of external effects.
            let mut observation = summary(
                &prepared.spec,
                ids.finished_observation.clone(),
                ids.basis.clone(),
            );
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
        observation: Some(observation),
        recorded,
        recording_failure,
        stale_receipt_attempts: writer.stale,
        reconciled_receipt_failures: writer.reconciled,
    })
}

#[cfg(test)]
#[path = "supervisor_tests.rs"]
mod tests;
