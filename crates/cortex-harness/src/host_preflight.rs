//! Bounded read-only Linux metadata for planning host qualification.
//!
//! This observer does not activate namespaces, write cgroup files, launch an
//! executable, inspect credentials, or confer execution authority. Observed
//! limits describe the observer's shared scope, never an owned attempt boundary.
//! Metadata availability is not behavioral qualification or delegation.

use std::{
    fs::{self, Metadata, OpenOptions},
    io::{self, Read},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::{Component, Path, PathBuf},
    time::{Duration, Instant},
};

const FILE_BYTES: usize = 16_384;
const TOTAL_BYTES: usize = 65_536;
const READS: usize = 32;
const DEADLINE: Duration = Duration::from_millis(1_000);
// Linux open(2) ABI constants. No unsafe code or privileged syscall is used.
const NO_FOLLOW_NONBLOCK: i32 = 0x20000 | 0x800;

/// One captured metadata value, or its precise bounded-read failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Observed<T> {
    /// Successfully parsed metadata; not a qualification or permission claim.
    Value(T),
    /// No value established; never interpreted as an unlimited or zero value.
    Unavailable(ReadIssue),
}

/// Redacted failure category; no original metadata or filesystem path is echoed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadIssue {
    /// The selected metadata file was absent.
    Missing,
    /// The operating system denied inspection.
    PermissionDenied,
    /// Metadata was malformed, ambiguous, or outside the supported shape.
    Malformed,
    /// A metadata byte/read limit was reached.
    Capacity,
    /// The cooperative observation deadline elapsed.
    Deadline,
    /// A symlink, nonregular file, or unsupported mount was encountered.
    UnsupportedFile,
    /// Identity or cgroup membership changed during inspection.
    Changed,
    /// Another local I/O category, without its private message.
    Io(io::ErrorKind),
}

/// Parsed Linux cgroup ceiling; a configured maximum is not measured enforcement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ceiling {
    /// Literal `max`; no finite ceiling is configured in this field.
    Unlimited,
    /// Literal numeric ceiling, including zero where supported by the interface.
    Limited(u64),
}

/// Observed CPU rate configuration, distinct from cumulative CPU execution time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CpuMax {
    /// Quota in microseconds, or no finite quota.
    pub quota: Ceiling,
    /// Positive quota period in microseconds.
    pub period_micros: u64,
}

/// Presence of the three controllers relevant to the current qualification plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Controllers {
    /// CPU controller name observed.
    pub cpu: bool,
    /// Memory controller name observed.
    pub memory: bool,
    /// Process/thread controller name observed.
    pub pids: bool,
}

/// The observer's current cgroup metadata. No field establishes delegation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CgroupMetadata {
    /// Number of normal path components below the cgroup root; names are omitted.
    pub depth: Observed<usize>,
    /// Exact supported cgroup-v2 mount layout was present.
    pub unified_mount: Observed<bool>,
    /// Controller names offered to children of this shared scope.
    pub controllers: Observed<Controllers>,
    /// Controller names enabled for children of this shared scope.
    pub subtree_control: Observed<Controllers>,
    /// Current scope CPU rate configuration.
    pub cpu_max: Observed<CpuMax>,
    /// Current scope charged-memory configuration, in bytes.
    pub memory_max: Observed<Ceiling>,
    /// Current scope swap configuration, in bytes.
    pub swap_max: Observed<Ceiling>,
    /// Current scope process/thread configuration.
    pub pids_max: Observed<Ceiling>,
    /// Whether the observed scope reports itself or descendants populated.
    pub populated: Observed<bool>,
    /// Matching before/after membership observations, not an atomic snapshot.
    pub membership_consistent: bool,
}

/// Namespace configuration observations. Nonzero values do not prove creation works.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamespaceMetadata {
    /// Configured maximum user namespaces.
    pub max_user_namespaces: Observed<u64>,
    /// Optional Linux distribution switch; absence leaves the switch unknown.
    pub unprivileged_userns_clone: Observed<bool>,
}

/// Bubblewrap file metadata only. No version, digest, execution, or trust claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExecutableMetadata {
    /// File length in bytes, not a memory budget.
    pub bytes: u64,
    /// File owner UID, not an operational principal authorization.
    pub owner_uid: u32,
    /// File mode bits including executable/set-ID flags.
    pub mode: u32,
    /// Device number for private correspondence inspection.
    pub device: u64,
    /// Inode number for private correspondence inspection.
    pub inode: u64,
}

/// Mandatory boundary whose qualification still needs actual observations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostControl {
    /// Allowed/denied filesystem operations and retained-handle boundaries.
    Filesystem,
    /// Parent/descendant network and mediation denials.
    Network,
    /// CPU rate enforcement on an owned process set.
    CpuRate,
    /// Cumulative CPU execution accounting and termination.
    CpuTime,
    /// Charged memory and no-swap enforcement.
    MemorySwap,
    /// Process/thread count and complete descendant ownership.
    Processes,
    /// Scratch byte, file and inode enforcement.
    Scratch,
    /// Protected verification/recovery capacity.
    VerificationReserve,
    /// Cancellation, cleanup and revocation of the complete process set.
    Cancellation,
    /// Synthetic credential-substitute confinement and mediation.
    Credentials,
}

/// A prerequisite or evidence gap, never an execution grant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GapReason {
    /// This read-only inspector does not establish an owned, delegated attempt scope.
    DelegationUnestablished,
    /// Required controller metadata is absent in the observer's current scope.
    ControllerMissing,
    /// Required metadata could not be parsed, boundedly read, or kept correspondent.
    MetadataUnavailable,
    /// Namespace metadata explicitly disables the requested rootless mechanism.
    NamespaceDisabled,
    /// Bubblewrap's fixed-path executable metadata was unavailable or unsupported.
    BackendUnavailable,
    /// No implemented/qualified quota mechanism is established by this observer.
    MechanismUnestablished,
    /// Actual denial/allow-neighbor behavior was not executed by this observer.
    BehaviorNotRun,
}

/// Causal gaps for one required control. Actual Evidence assessments remain external.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlGap {
    /// Required host boundary.
    pub control: HostControl,
    /// Complete applicable causes, in stable order.
    pub reasons: Vec<GapReason>,
}

/// Read-only metadata and causal qualification gaps, with no operational token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostPreflight {
    /// Current observer-scope metadata, not an attempt resource reservation.
    pub cgroup: CgroupMetadata,
    /// Configuration only; namespace activation was not attempted.
    pub namespaces: NamespaceMetadata,
    /// Fixed `/usr/bin/bwrap` metadata only; the executable is never invoked.
    pub bubblewrap: Observed<ExecutableMetadata>,
    /// Metadata reads attempted, including refused reads.
    pub reads: usize,
    /// Metadata bytes actually consumed; no raw contents are retained in this report.
    pub bytes_read: usize,
    /// Measured cooperative observer elapsed milliseconds.
    pub elapsed_ms: u64,
}

impl HostPreflight {
    /// Enumerate missing prerequisites separately from unexecuted behavior.
    /// Presence of every metadata field still leaves delegation and behavior open.
    /// These reasons cannot be consumed as permission to create a process.
    pub fn control_gaps(&self) -> Vec<ControlGap> {
        use GapReason::*;
        use HostControl::*;
        [
            Filesystem,
            Network,
            CpuRate,
            CpuTime,
            MemorySwap,
            Processes,
            Scratch,
            VerificationReserve,
            Cancellation,
            Credentials,
        ]
        .into_iter()
        .map(|control| {
            let mut reasons = vec![DelegationUnestablished];
            if matches!(control, Filesystem | Network | Credentials) {
                if !matches!(self.bubblewrap, Observed::Value(_)) {
                    reasons.push(BackendUnavailable);
                }
                if self.namespaces.max_user_namespaces == Observed::Value(0)
                    || self.namespaces.unprivileged_userns_clone == Observed::Value(false)
                {
                    reasons.push(NamespaceDisabled);
                }
                if matches!(
                    self.namespaces.max_user_namespaces,
                    Observed::Unavailable(_)
                ) {
                    reasons.push(MetadataUnavailable);
                }
            }
            let controller = match control {
                CpuRate | CpuTime => Some(0),
                MemorySwap => Some(1),
                Processes | Cancellation => Some(2),
                _ => None,
            };
            if let Some(which) = controller {
                match &self.cgroup.controllers {
                    Observed::Value(c) => {
                        if ![c.cpu, c.memory, c.pids][which] {
                            reasons.push(ControllerMissing);
                        }
                    }
                    Observed::Unavailable(_) => reasons.push(MetadataUnavailable),
                }
            }
            if matches!(
                control,
                CpuTime | Scratch | VerificationReserve | Credentials
            ) {
                reasons.push(MechanismUnestablished);
            }
            reasons.push(BehaviorNotRun);
            ControlGap { control, reasons }
        })
        .collect()
    }
}

struct Reader {
    start: Instant,
    reads: usize,
    bytes: usize,
}

impl Reader {
    fn new() -> Self {
        Self {
            start: Instant::now(),
            reads: 0,
            bytes: 0,
        }
    }

    fn guard(&mut self) -> Result<(), ReadIssue> {
        self.reads += 1;
        if self.reads > READS || self.bytes >= TOTAL_BYTES {
            return Err(ReadIssue::Capacity);
        }
        if self.start.elapsed() >= DEADLINE {
            return Err(ReadIssue::Deadline);
        }
        Ok(())
    }

    fn read(&mut self, path: &Path) -> Result<String, ReadIssue> {
        self.guard()?;
        let before = fs::symlink_metadata(path).map_err(issue)?;
        if !before.is_file() {
            return Err(ReadIssue::UnsupportedFile);
        }
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(NO_FOLLOW_NONBLOCK)
            .open(path)
            .map_err(issue)?;
        if stamp(&before) != stamp(&file.metadata().map_err(issue)?) {
            return Err(ReadIssue::Changed);
        }
        let budget = FILE_BYTES.min(TOTAL_BYTES - self.bytes);
        let mut bytes = Vec::new();
        let read_result = (&file).take((budget + 1) as u64).read_to_end(&mut bytes);
        self.bytes += bytes.len();
        read_result.map_err(issue)?;
        if bytes.len() > budget {
            return Err(ReadIssue::Capacity);
        }
        if stamp(&before) != stamp(&file.metadata().map_err(issue)?)
            || stamp(&before) != stamp(&fs::symlink_metadata(path).map_err(issue)?)
        {
            return Err(ReadIssue::Changed);
        }
        if self.start.elapsed() >= DEADLINE {
            return Err(ReadIssue::Deadline);
        }
        String::from_utf8(bytes).map_err(|_| ReadIssue::Malformed)
    }

    fn parsed<T>(&mut self, path: &Path, parse: fn(&str) -> Result<T, ReadIssue>) -> Observed<T> {
        observed(self.read(path).and_then(|s| parse(&s)))
    }
}

fn issue(e: io::Error) -> ReadIssue {
    match e.kind() {
        io::ErrorKind::NotFound => ReadIssue::Missing,
        io::ErrorKind::PermissionDenied => ReadIssue::PermissionDenied,
        kind => ReadIssue::Io(kind),
    }
}

fn observed<T>(r: Result<T, ReadIssue>) -> Observed<T> {
    match r {
        Ok(v) => Observed::Value(v),
        Err(e) => Observed::Unavailable(e),
    }
}

fn stamp(m: &Metadata) -> (u64, u64, u64, u32, i64, i64, i64, i64) {
    (
        m.dev(),
        m.ino(),
        m.len(),
        m.mode(),
        m.mtime(),
        m.mtime_nsec(),
        m.ctime(),
        m.ctime_nsec(),
    )
}

fn number(s: &str) -> Result<u64, ReadIssue> {
    let s = s.trim();
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return Err(ReadIssue::Malformed);
    }
    s.parse().map_err(|_| ReadIssue::Malformed)
}

fn boolean(s: &str) -> Result<bool, ReadIssue> {
    match number(s)? {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(ReadIssue::Malformed),
    }
}

fn ceiling(s: &str) -> Result<Ceiling, ReadIssue> {
    if s.trim() == "max" {
        Ok(Ceiling::Unlimited)
    } else {
        number(s).map(Ceiling::Limited)
    }
}

fn cpu_max(s: &str) -> Result<CpuMax, ReadIssue> {
    let fields = s.split_whitespace().collect::<Vec<_>>();
    if fields.len() != 2 {
        return Err(ReadIssue::Malformed);
    }
    let quota = ceiling(fields[0])?;
    let period_micros = number(fields[1])?;
    if period_micros == 0 || quota == Ceiling::Limited(0) {
        return Err(ReadIssue::Malformed);
    }
    Ok(CpuMax {
        quota,
        period_micros,
    })
}

fn controllers(s: &str) -> Result<Controllers, ReadIssue> {
    let mut seen = std::collections::BTreeSet::new();
    for item in s.split_whitespace() {
        if item.len() > 32
            || !item.bytes().all(|b| b.is_ascii_lowercase() || b == b'_')
            || !seen.insert(item)
            || seen.len() > 32
        {
            return Err(ReadIssue::Malformed);
        }
    }
    Ok(Controllers {
        cpu: seen.contains("cpu"),
        memory: seen.contains("memory"),
        pids: seen.contains("pids"),
    })
}

fn populated(s: &str) -> Result<bool, ReadIssue> {
    let mut found = None;
    let mut seen = std::collections::BTreeSet::new();
    for line in s.lines() {
        let fields = line.split_whitespace().collect::<Vec<_>>();
        if fields.len() != 2 || !seen.insert(fields[0]) {
            return Err(ReadIssue::Malformed);
        }
        let value = boolean(fields[1])?;
        if fields[0] == "populated" {
            found = Some(value);
        }
    }
    found.ok_or(ReadIssue::Malformed)
}

fn membership(s: &str) -> Result<PathBuf, ReadIssue> {
    let lines = s.lines().collect::<Vec<_>>();
    if lines.len() != 1 {
        return Err(ReadIssue::Malformed);
    }
    let raw = lines[0].strip_prefix("0::").ok_or(ReadIssue::Malformed)?;
    if !raw.starts_with('/')
        || raw.len() > 4096
        || raw.contains("//")
        || (raw.len() > 1 && raw.ends_with('/'))
        || raw.bytes().any(|b| b.is_ascii_control())
    {
        return Err(ReadIssue::Malformed);
    }
    let path = Path::new(raw);
    if path
        .components()
        .any(|c| !matches!(c, Component::RootDir | Component::Normal(_)))
        || raw.split('/').any(|c| c == "." || c == "..")
    {
        return Err(ReadIssue::Malformed);
    }
    Ok(path
        .strip_prefix("/")
        .map_err(|_| ReadIssue::Malformed)?
        .to_owned())
}

fn mount(s: &str) -> Result<bool, ReadIssue> {
    let mut found = 0;
    for line in s.lines() {
        let fields = line.split_whitespace().collect::<Vec<_>>();
        if fields.len() < 10 {
            return Err(ReadIssue::Malformed);
        }
        if fields[4] == "/sys/fs/cgroup" {
            found += 1;
            let separator = fields
                .iter()
                .position(|v| *v == "-")
                .ok_or(ReadIssue::Malformed)?;
            if fields[3] != "/" || fields.get(separator + 1) != Some(&"cgroup2") {
                return Err(ReadIssue::UnsupportedFile);
            }
        }
    }
    if found == 1 {
        Ok(true)
    } else {
        Err(ReadIssue::UnsupportedFile)
    }
}

fn directory(root: &Path, rel: &Path) -> Result<PathBuf, ReadIssue> {
    let mut path = root.to_owned();
    if !fs::symlink_metadata(&path).map_err(issue)?.is_dir() {
        return Err(ReadIssue::UnsupportedFile);
    }
    for component in rel.components() {
        if !matches!(component, Component::Normal(_)) {
            return Err(ReadIssue::Malformed);
        }
        path.push(component);
        if !fs::symlink_metadata(&path).map_err(issue)?.is_dir() {
            return Err(ReadIssue::UnsupportedFile);
        }
    }
    Ok(path)
}

fn executable(path: &Path) -> Observed<ExecutableMetadata> {
    observed((|| {
        let m = fs::symlink_metadata(path).map_err(issue)?;
        if !m.is_file() || m.mode() & 0o111 == 0 || m.len() == 0 {
            return Err(ReadIssue::UnsupportedFile);
        }
        Ok(ExecutableMetadata {
            bytes: m.len(),
            owner_uid: m.uid(),
            mode: m.mode(),
            device: m.dev(),
            inode: m.ino(),
        })
    })())
}

fn missing_cgroup(reason: ReadIssue) -> CgroupMetadata {
    CgroupMetadata {
        depth: Observed::Unavailable(reason),
        unified_mount: Observed::Unavailable(reason),
        controllers: Observed::Unavailable(reason),
        subtree_control: Observed::Unavailable(reason),
        cpu_max: Observed::Unavailable(reason),
        memory_max: Observed::Unavailable(reason),
        swap_max: Observed::Unavailable(reason),
        pids_max: Observed::Unavailable(reason),
        populated: Observed::Unavailable(reason),
        membership_consistent: false,
    }
}

/// Observe fixed Linux metadata without subprocesses or mutation. The bound is
/// 32 reads, 16 KiB per file and 64 KiB total, plus one overflow-detection byte.
/// The one-second deadline is cooperative on responsive procfs/sysfs, not a hard
/// interruption guarantee. A changed membership invalidates all cgroup values.
/// No home, repository, credential or prior session contents are inspected.
pub fn inspect_host() -> HostPreflight {
    inspect_at(
        Path::new("/proc"),
        Path::new("/sys/fs/cgroup"),
        Path::new("/usr/bin/bwrap"),
        || {},
    )
}

fn inspect_at(
    proc_root: &Path,
    cgroup_root: &Path,
    bwrap: &Path,
    before_recheck: impl FnOnce(),
) -> HostPreflight {
    let mut reader = Reader::new();
    let member_file = proc_root.join("self/cgroup");
    let first = reader.read(&member_file).and_then(|s| membership(&s));
    let unified_mount = reader.parsed(&proc_root.join("self/mountinfo"), mount);
    let mut cgroup = match &first {
        Ok(rel) if unified_mount == Observed::Value(true) => match directory(cgroup_root, rel) {
            Ok(dir) => CgroupMetadata {
                depth: Observed::Value(rel.components().count()),
                unified_mount: unified_mount.clone(),
                controllers: reader.parsed(&dir.join("cgroup.controllers"), controllers),
                subtree_control: reader.parsed(&dir.join("cgroup.subtree_control"), controllers),
                cpu_max: reader.parsed(&dir.join("cpu.max"), cpu_max),
                memory_max: reader.parsed(&dir.join("memory.max"), ceiling),
                swap_max: reader.parsed(&dir.join("memory.swap.max"), ceiling),
                pids_max: reader.parsed(&dir.join("pids.max"), ceiling),
                populated: reader.parsed(&dir.join("cgroup.events"), populated),
                membership_consistent: false,
            },
            Err(e) => missing_cgroup(e),
        },
        Ok(_) => missing_cgroup(match unified_mount {
            Observed::Unavailable(e) => e,
            _ => ReadIssue::Malformed,
        }),
        Err(e) => missing_cgroup(*e),
    };
    cgroup.unified_mount = unified_mount;
    let namespaces = NamespaceMetadata {
        max_user_namespaces: reader.parsed(&proc_root.join("sys/user/max_user_namespaces"), number),
        unprivileged_userns_clone: reader.parsed(
            &proc_root.join("sys/kernel/unprivileged_userns_clone"),
            boolean,
        ),
    };
    let bubblewrap = executable(bwrap);
    before_recheck();
    let second = reader.read(&member_file).and_then(|s| membership(&s));
    match (&first, &second) {
        (Ok(a), Ok(b)) if a == b => cgroup.membership_consistent = true,
        (Ok(_), _) => cgroup = missing_cgroup(ReadIssue::Changed),
        _ => {}
    }
    HostPreflight {
        cgroup,
        namespaces,
        bubblewrap,
        reads: reader.reads,
        bytes_read: reader.bytes,
        elapsed_ms: u64::try_from(reader.start.elapsed().as_millis()).unwrap_or(u64::MAX),
    }
}

#[cfg(test)]
mod tests;
