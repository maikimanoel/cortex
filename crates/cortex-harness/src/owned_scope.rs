//! Narrow, owned systemd scope adapter for audited provider-free fixtures.
//!
//! A prepared command is not authority. The existing supervisor admits the
//! exact intent before spawning it and withholds input until current scope
//! correspondence is observed. Namespace flags and metadata are not a claim of
//! qualification. Manager operations assume a cooperating owner: systemd's
//! unit-name interface cannot atomically predicate a signal on InvocationID.

use crate::supervisor::{HarnessError, PinnedExecutable};
use cortex_core::program::supervision::*;
use std::{
    collections::BTreeMap,
    fs::{self, Metadata, OpenOptions},
    io::{self, Read},
    os::{
        fd::OwnedFd,
        unix::{
            fs::{FileTypeExt, MetadataExt, OpenOptionsExt},
            net::UnixStream,
        },
    },
    path::{Component, Path, PathBuf},
    process::{Child, Command, ExitStatus, Stdio},
    thread,
    time::{Duration, Instant},
};

const METADATA_BYTES: u64 = 16_384;
const HELPER_BYTES: usize = 16_384;
const HELPER_TIME: Duration = Duration::from_millis(1_000);
const POLL: Duration = Duration::from_millis(2);
// Linux O_NOFOLLOW | O_NONBLOCK, matching the fixed host-preflight reader.
// These refuse final symlinks and prevent a raced FIFO from blocking open.
const NO_FOLLOW_NONBLOCK: i32 = 0x20000 | 0x800;
const ROOT_FILES: [&str; 6] = [
    "cgroup.controllers",
    "cgroup.subtree_control",
    "cpu.max",
    "memory.max",
    "memory.swap.max",
    "pids.max",
];

/// Only the exact owned scope can be targeted; there is no arbitrary PID API.
pub(crate) enum ScopeSignal {
    Terminate,
    Kill,
}

#[derive(Clone, PartialEq, Eq)]
struct Node {
    device: u64,
    inode: u64,
    uid: u32,
    mode: u32,
}
fn node(m: &Metadata) -> Node {
    Node {
        device: m.dev(),
        inode: m.ino(),
        uid: m.uid(),
        mode: m.mode(),
    }
}

struct FilePin {
    path: PathBuf,
    metadata: Node,
    bytes: Vec<u8>,
}

struct PendingHelper {
    child: Child,
    identity: ScopeHelperIdentity,
}
impl FilePin {
    fn inspect(path: &Path, limit: u64) -> Result<Self, HarnessError> {
        let m = fs::symlink_metadata(path)?;
        if !m.is_file() || m.len() > limit || fs::canonicalize(path)? != path {
            return Err(HarnessError::TargetChanged);
        }
        let mut f = OpenOptions::new()
            .read(true)
            .custom_flags(NO_FOLLOW_NONBLOCK)
            .open(path)?;
        if node(&f.metadata()?) != node(&m) {
            return Err(HarnessError::TargetChanged);
        }
        let mut bytes = Vec::new();
        (&mut f)
            .take(limit.checked_add(1).ok_or(HarnessError::Limits)?)
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 != m.len() || node(&f.metadata()?) != node(&m) {
            return Err(HarnessError::TargetChanged);
        }
        Ok(Self {
            path: path.to_owned(),
            metadata: node(&m),
            bytes,
        })
    }
    fn check(&self) -> Result<(), HarnessError> {
        let current = Self::inspect(&self.path, self.bytes.len() as u64)?;
        if current.metadata != self.metadata || current.bytes != self.bytes {
            return Err(HarnessError::TargetChanged);
        }
        Ok(())
    }
}

pub(crate) struct PreparedScope {
    spec: OwnedScopeSpec,
    process: ProcessSpec,
    launcher: PinnedExecutable,
    manager: PinnedExecutable,
    bubblewrap: Option<PinnedExecutable>,
    mounts: Vec<FilePin>,
    root: Node,
    root_values: Vec<String>,
    runtime: Node,
    last: Option<ScopeObservation>,
    pending_helpers: Vec<PendingHelper>,
}

impl PreparedScope {
    /// Filesystem-only preparation. It neither starts an auxiliary process nor
    /// creates a scope or releases prepared package bytes.
    pub(crate) fn inspect(process: &ProcessSpec, deadline: Instant) -> Result<Self, HarnessError> {
        let spec = process.containment.clone().ok_or(HarnessError::Contract)?;
        validate_unit(&spec.unit)?;
        let root = inspect_root(&spec.delegated_root)?;
        let root_values = root_values(&spec.delegated_root)?;
        require_controllers(&root_values)?;
        let runtime = inspect_runtime(&spec.runtime_dir)?;
        if root.uid != runtime.uid {
            return Err(HarnessError::Contract);
        }
        for path in [&process.executable, &spec.launcher, &spec.manager] {
            refuse_privilege_bits(path)?;
        }
        let launcher = PinnedExecutable::inspect(&spec.launcher, process.limits.executable_bytes)?;
        let manager = PinnedExecutable::inspect(&spec.manager, process.limits.executable_bytes)?;
        let mut mounts = Vec::new();
        let bubblewrap = match &spec.isolation {
            ScopeIsolation::ScopeOnlyInertFixture => None,
            ScopeIsolation::RootlessSyntheticCanary {
                bubblewrap,
                mounts: selected,
                ..
            } => {
                refuse_privilege_bits(bubblewrap)?;
                validate_mounts(selected)?;
                let mut remaining = process.limits.executable_bytes;
                for m in selected {
                    let pin = FilePin::inspect(&m.source, remaining)?;
                    remaining = remaining
                        .checked_sub(pin.bytes.len() as u64)
                        .ok_or(HarnessError::Limits)?;
                    mounts.push(pin);
                }
                Some(PinnedExecutable::inspect(
                    bubblewrap,
                    process.limits.executable_bytes,
                )?)
            }
        };
        let mut prepared = Self {
            spec,
            process: process.clone(),
            launcher,
            manager,
            bubblewrap,
            mounts,
            root,
            root_values,
            runtime,
            last: None,
            pending_helpers: Vec::new(),
        };
        prepared.recheck(deadline)?;
        Ok(prepared)
    }

    /// Bounded manager queries run only after the canonical dispatch intent is
    /// durable. The caller must retain this object on failure and capture any
    /// unresolved helper identities; a query timeout is not effect-free.
    pub(crate) fn prepare_dispatch(&mut self, deadline: Instant) -> Result<(), HarnessError> {
        self.recheck(deadline)?;
        let manager_root = self.query("-.slice", "ControlGroup,DisableControllers", deadline)?;
        let root_properties = properties(&manager_root.output)?;
        if !manager_root.status.success()
            || root_properties
                .get("DisableControllers")
                .is_none_or(|s| !s.is_empty())
            || root_properties
                .get("ControlGroup")
                .map(|s| cgroup_path(s))
                .transpose()?
                != Some(self.spec.delegated_root.clone())
        {
            return Err(HarnessError::Contract);
        }
        let unit = self.spec.unit.clone();
        let previous = self.query(&unit, "LoadState", deadline)?;
        if !previous.status.success()
            || properties(&previous.output)?
                .get("LoadState")
                .map(String::as_str)
                != Some("not-found")
        {
            return Err(HarnessError::TargetChanged);
        }
        match fs::symlink_metadata(self.spec.delegated_root.join(&self.spec.unit)) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
            Ok(_) => return Err(HarnessError::TargetChanged),
        }
        self.recheck(deadline)
    }

    pub(crate) fn recheck(&mut self, deadline: Instant) -> Result<(), HarnessError> {
        self.reap_helpers(deadline)?;
        if Instant::now() >= deadline {
            return Err(HarnessError::Io(io::ErrorKind::TimedOut));
        }
        self.launcher.check()?;
        self.manager.check()?;
        for path in [
            &self.process.executable,
            &self.spec.launcher,
            &self.spec.manager,
        ] {
            refuse_privilege_bits(path)?;
        }
        if let Some(pin) = &self.bubblewrap {
            pin.check()?;
        }
        if let ScopeIsolation::RootlessSyntheticCanary { bubblewrap, .. } = &self.spec.isolation {
            refuse_privilege_bits(bubblewrap)?;
        }
        for pin in &self.mounts {
            pin.check()?;
        }
        if inspect_root(&self.spec.delegated_root)? != self.root
            || root_values(&self.spec.delegated_root)? != self.root_values
            || inspect_runtime(&self.spec.runtime_dir)? != self.runtime
        {
            return Err(HarnessError::TargetChanged);
        }
        Ok(())
    }

    /// No child is created here. No shell, caller environment or PATH lookup is
    /// used. ScopeOnlyInertFixture is explicitly trusted; the controller supplies two
    /// non-secret manager routing variables. The selected manager may add its
    /// generated INVOCATION_ID; fixtures must validate that non-secret identity
    /// explicitly. The namespace profile clears these variables.
    pub(crate) fn command(&self, process: &ProcessSpec) -> Result<Command, HarnessError> {
        if process != &self.process {
            return Err(HarnessError::TargetChanged);
        }
        let mut cmd = manager_command(&self.spec.launcher, &self.spec.runtime_dir);
        cmd.args(scope_arguments(process)?);
        cmd.current_dir(&process.cwd);
        Ok(cmd)
    }

    /// Retained last-known identity and counters are never relabeled current
    /// when a query fails or the unit disappears.
    pub(crate) fn last_observation(&self) -> Option<ScopeObservation> {
        self.last.clone()
    }

    pub(crate) fn unresolved_helpers(&self) -> Vec<ScopeHelperIdentity> {
        self.pending_helpers
            .iter()
            .map(|p| p.identity.clone())
            .collect()
    }

    pub(crate) fn observe(
        &mut self,
        process: Option<&ProcessIdentity>,
        deadline: Instant,
    ) -> Result<ScopeObservation, HarnessError> {
        if let Some(last) = &mut self.last {
            last.controls = None;
            last.populated = None;
            last.lifecycle_verified = false;
        }
        let result = self.observe_current(process, deadline);
        if let Some(last) = &mut self.last {
            last.helper_quiescent = self.pending_helpers.is_empty();
        }
        result
    }

    fn observe_current(
        &mut self,
        process: Option<&ProcessIdentity>,
        deadline: Instant,
    ) -> Result<ScopeObservation, HarnessError> {
        self.recheck(deadline)?;
        let unit = self.spec.unit.clone();
        let query = self.query(&unit, "LoadState,InvocationID,ControlGroup,RuntimeMaxUSec,TimeoutStopUSec,SendSIGKILL,KillMode,KillSignal,FinalKillSignal", deadline)?;
        if !query.status.success() {
            return Err(HarnessError::TargetChanged);
        }
        let values = properties(&query.output)?;
        if values.get("LoadState").map(String::as_str) != Some("loaded") {
            return Err(HarnessError::TargetChanged);
        }
        let invocation = values.get("InvocationID").ok_or(HarnessError::Contract)?;
        if invocation.len() != 32
            || !invocation
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            || invocation.bytes().all(|b| b == b'0')
        {
            return Err(HarnessError::Contract);
        }
        let group = cgroup_path(values.get("ControlGroup").ok_or(HarnessError::Contract)?)?;
        if group != self.spec.delegated_root.join(&self.spec.unit) {
            return Err(HarnessError::TargetChanged);
        }
        let before = inspect_root(&group)?;
        let identity = ScopeIdentity {
            spec: self.spec.reference.clone(),
            unit: self.spec.unit.clone(),
            invocation_id: invocation.clone(),
            cgroup: group.clone(),
            device: before.device,
            inode: before.inode,
        };
        if self
            .last
            .as_ref()
            .is_some_and(|old| old.identity != identity)
        {
            return Err(HarnessError::TargetChanged);
        }
        // Retain the actual incarnation even if subsequent controls are absent.
        let mut observation = self.last.clone().unwrap_or(ScopeObservation {
            identity,
            controls: None,
            membership_verified: false,
            populated: None,
            cpu_usage_us: None,
            memory_peak_bytes: None,
            tasks_peak: None,
            helper_quiescent: true,
            lifecycle_verified: false,
        });
        self.last = Some(observation.clone());
        verify_lifecycle(&values, &self.process)?;
        let controls = read_controls(&group)?;
        if controls != self.spec.controls {
            return Err(HarnessError::TargetChanged);
        }
        let events = key_values(&read_metadata(&group.join("cgroup.events"))?)?;
        let populated = match events.get("populated").copied() {
            Some(0) => false,
            Some(1) => true,
            _ => return Err(HarnessError::Contract),
        };
        let cpu = key_values(&read_metadata(&group.join("cpu.stat"))?)?
            .get("usage_usec")
            .copied()
            .ok_or(HarnessError::Contract)?;
        if observation.cpu_usage_us.is_some_and(|old| cpu < old) {
            return Err(HarnessError::TargetChanged);
        }
        observation.cpu_usage_us = Some(cpu);
        observation.memory_peak_bytes =
            optional_counter(&group.join("memory.peak"), observation.memory_peak_bytes)?;
        observation.tasks_peak =
            optional_counter(&group.join("pids.peak"), observation.tasks_peak)?;
        if let Some(process) = process {
            verify_membership(process, &group)?;
            observation.membership_verified = true;
        }
        if inspect_root(&group)? != before
            || root_values(&self.spec.delegated_root)? != self.root_values
        {
            return Err(HarnessError::TargetChanged);
        }
        observation.controls = Some(controls);
        observation.populated = Some(populated);
        observation.helper_quiescent = self.pending_helpers.is_empty();
        observation.lifecycle_verified = true;
        self.last = Some(observation.clone());
        Ok(observation)
    }

    pub(crate) fn terminate(
        &mut self,
        signal: ScopeSignal,
        deadline: Instant,
    ) -> Result<(), HarnessError> {
        let expected = self
            .last
            .as_ref()
            .ok_or(HarnessError::Contract)?
            .identity
            .clone();
        // Identity inspection must not consume the complete signal phase. Both
        // operations share the original phase deadline; no allowance is reset.
        let now = Instant::now();
        let remaining = deadline.saturating_duration_since(now);
        if remaining.is_zero() {
            return Err(HarnessError::Io(io::ErrorKind::TimedOut));
        }
        let inspection_deadline = now + remaining / 2;
        if self.observe(None, inspection_deadline)?.identity != expected {
            return Err(HarnessError::TargetChanged);
        }
        let signal = match signal {
            ScopeSignal::Terminate => "--signal=SIGTERM",
            ScopeSignal::Kill => "--signal=SIGKILL",
        };
        let args = [
            "--user",
            "--no-pager",
            "kill",
            signal,
            "--kill-whom=all",
            "--",
            &self.spec.unit,
        ];
        let args: Vec<String> = args.into_iter().map(str::to_owned).collect();
        if !self.helper(&args, deadline)?.status.success() {
            return Err(HarnessError::Io(io::ErrorKind::Other));
        }
        Ok(())
    }

    fn query(
        &mut self,
        unit: &str,
        fields: &str,
        deadline: Instant,
    ) -> Result<HelperOutput, HarnessError> {
        self.helper(
            &[
                "--user".into(),
                "--no-pager".into(),
                "show".into(),
                format!("--property={fields}"),
                "--".into(),
                unit.into(),
            ],
            deadline,
        )
    }
    fn helper(&mut self, args: &[String], deadline: Instant) -> Result<HelperOutput, HarnessError> {
        self.reap_helpers(deadline)?;
        self.manager.check()?;
        if Instant::now() >= deadline {
            return Err(HarnessError::Io(io::ErrorKind::TimedOut));
        }
        let mut command = manager_command(&self.spec.manager, &self.spec.runtime_dir);
        command.args(args);
        let result = run_helper(command, deadline, &mut self.pending_helpers);
        if let Some(last) = &mut self.last {
            last.helper_quiescent = self.pending_helpers.is_empty();
        }
        result
    }
    fn reap_helpers(&mut self, deadline: Instant) -> Result<(), HarnessError> {
        while let Some(mut pending) = self.pending_helpers.pop() {
            loop {
                match pending.child.try_wait() {
                    Ok(Some(_)) => break,
                    Ok(None) if Instant::now() < deadline => {
                        thread::sleep(POLL.min(deadline.saturating_duration_since(Instant::now())))
                    }
                    _ => {
                        self.pending_helpers.push(pending);
                        return Err(HarnessError::Io(io::ErrorKind::TimedOut));
                    }
                }
            }
        }
        Ok(())
    }
}

fn manager_command(binary: &Path, runtime: &Path) -> Command {
    let mut command = Command::new(binary);
    command
        .env_clear()
        .current_dir("/")
        .stdin(Stdio::null())
        .arg("--no-ask-password");
    command.env("XDG_RUNTIME_DIR", runtime).env(
        "DBUS_SESSION_BUS_ADDRESS",
        format!("unix:path={}/bus", runtime.display()),
    );
    command
}

fn scope_arguments(process: &ProcessSpec) -> Result<Vec<String>, HarnessError> {
    let s = process.containment.as_ref().ok_or(HarnessError::Contract)?;
    let c = &s.controls;
    validate_unit(&s.unit)?;
    let percent = quota_percent(c.cpu_quota_us, c.cpu_period_us)?;
    let mut args = vec![
        "--user".into(),
        "--scope".into(),
        "--quiet".into(),
        format!("--unit={}", s.unit),
        "--slice=-.slice".into(),
        "--expand-environment=no".into(),
        format!("--property=CPUQuota={percent}"),
        format!("--property=CPUQuotaPeriodSec={}us", c.cpu_period_us),
        format!("--property=MemoryMax={}", c.memory_bytes),
        format!("--property=MemorySwapMax={}", c.swap_bytes),
        format!("--property=TasksMax={}", c.tasks_max),
        format!("--property=RuntimeMaxSec={}ms", process.limits.wall_ms),
        "--property=RuntimeRandomizedExtraSec=0".into(),
        "--property=KillMode=control-group".into(),
        format!("--property=TimeoutStopSec={}ms", s.cancellation_grace_ms),
        "--property=SendSIGKILL=yes".into(),
        "--property=KillSignal=SIGTERM".into(),
        "--property=FinalKillSignal=SIGKILL".into(),
        "--".into(),
    ];
    match &s.isolation {
        ScopeIsolation::ScopeOnlyInertFixture => {
            args.push(path_text(&process.executable)?);
            args.extend(process.args.clone());
        }
        ScopeIsolation::RootlessSyntheticCanary {
            bubblewrap,
            mounts,
            scratch_bytes,
            ..
        } => {
            validate_mounts(mounts)?;
            args.push(path_text(bubblewrap)?);
            args.extend(
                [
                    "--unshare-user",
                    "--unshare-pid",
                    "--unshare-net",
                    "--unshare-ipc",
                    "--unshare-uts",
                    "--disable-userns",
                    "--clearenv",
                    "--new-session",
                    "--die-with-parent",
                    "--cap-drop",
                    "ALL",
                ]
                .into_iter()
                .map(str::to_owned),
            );
            for m in mounts {
                args.extend([
                    "--ro-bind".into(),
                    path_text(&m.source)?,
                    path_text(&m.destination)?,
                ]);
            }
            args.extend([
                "--ro-bind".into(),
                path_text(&process.executable)?,
                "/fixture".into(),
                "--size".into(),
                scratch_bytes.to_string(),
                "--tmpfs".into(),
                "/scratch".into(),
                "--remount-ro".into(),
                "/".into(),
                "--chdir".into(),
                "/scratch".into(),
                "--".into(),
                "/fixture".into(),
            ]);
            args.extend(process.args.clone());
        }
    }
    Ok(args)
}

fn quota_percent(quota: u64, period: u64) -> Result<String, HarnessError> {
    if quota == 0 || period == 0 {
        return Err(HarnessError::Contract);
    }
    // The selected manager interface accepts a positive signed-32-bit count of
    // hundredths of one percent. Preserve the exact ratio; never round a limit.
    // Widen before multiplying so representable ratios are not falsely refused.
    let scaled = u128::from(quota) * 10_000;
    let period = u128::from(period);
    if scaled % period != 0 {
        return Err(HarnessError::Contract);
    }
    let hundredths = scaled / period;
    if hundredths > i32::MAX as u128 {
        return Err(HarnessError::Limits);
    }
    Ok(format!("{}.{:02}%", hundredths / 100, hundredths % 100))
}

fn validate_unit(unit: &str) -> Result<(), HarnessError> {
    let id = unit
        .strip_prefix("cortex-qualification-")
        .and_then(|s| s.strip_suffix(".scope"))
        .ok_or(HarnessError::Contract)?;
    if id.is_empty()
        || id.len() > 64
        || !id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    {
        return Err(HarnessError::Contract);
    }
    Ok(())
}
fn path_text(path: &Path) -> Result<String, HarnessError> {
    path.to_str()
        .map(str::to_owned)
        .ok_or(HarnessError::Contract)
}
fn refuse_privilege_bits(path: &Path) -> Result<(), HarnessError> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.mode() & 0o6000 != 0 {
        return Err(HarnessError::TargetChanged);
    }
    // File capabilities need separate exact-binary qualification. Mode bits do
    // not establish their absence; this adapter makes no such inference.
    Ok(())
}
fn validate_mounts(mounts: &[ReadOnlyMount]) -> Result<(), HarnessError> {
    if mounts.len() > 16 {
        return Err(HarnessError::Limits);
    }
    for (i, m) in mounts.iter().enumerate() {
        if !plain_absolute(&m.source)
            || !plain_absolute(&m.destination)
            || m.destination == Path::new("/")
            || [
                "/fixture", "/scratch", "/proc", "/sys", "/dev", "/run", "/home",
            ]
            .iter()
            .any(|p| m.destination.starts_with(p))
            || mounts[..i].iter().any(|old| {
                old.destination.starts_with(&m.destination)
                    || m.destination.starts_with(&old.destination)
            })
        {
            return Err(HarnessError::Contract);
        }
    }
    Ok(())
}
fn plain_absolute(path: &Path) -> bool {
    path.is_absolute()
        && path
            .components()
            .all(|c| matches!(c, Component::RootDir | Component::Normal(_)))
}
fn inspect_root(path: &Path) -> Result<Node, HarnessError> {
    if !plain_absolute(path)
        || !path.starts_with("/sys/fs/cgroup")
        || path == Path::new("/sys/fs/cgroup")
    {
        return Err(HarnessError::Contract);
    }
    let m = fs::symlink_metadata(path)?;
    if !m.is_dir() || fs::canonicalize(path)? != path || m.ino() == 0 {
        return Err(HarnessError::TargetChanged);
    }
    Ok(node(&m))
}
fn inspect_runtime(path: &Path) -> Result<Node, HarnessError> {
    let text = path.to_str().ok_or(HarnessError::Contract)?;
    let uid = text
        .strip_prefix("/run/user/")
        .filter(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
        .ok_or(HarnessError::Contract)?
        .parse::<u32>()
        .map_err(|_| HarnessError::Contract)?;
    let m = fs::symlink_metadata(path)?;
    if !m.is_dir()
        || m.uid() != uid
        || m.mode() & 0o077 != 0
        || fs::canonicalize(path)? != path
        || !fs::symlink_metadata(path.join("bus"))?
            .file_type()
            .is_socket()
    {
        return Err(HarnessError::TargetChanged);
    }
    let status = read_metadata(Path::new("/proc/self/status"))?;
    let line = status
        .lines()
        .find_map(|line| line.strip_prefix("Uid:"))
        .ok_or(HarnessError::Contract)?;
    if line
        .split_whitespace()
        .take(4)
        .map(|p| p.parse::<u32>())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| HarnessError::Contract)?
        != vec![uid; 4]
    {
        return Err(HarnessError::Contract);
    }
    Ok(node(&m))
}
fn read_metadata(path: &Path) -> Result<String, HarnessError> {
    let m = fs::symlink_metadata(path)?;
    if !m.is_file() {
        return Err(HarnessError::TargetChanged);
    }
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(NO_FOLLOW_NONBLOCK)
        .open(path)?;
    if node(&file.metadata()?) != node(&m) {
        return Err(HarnessError::TargetChanged);
    }
    let mut bytes = Vec::new();
    (&mut file)
        .take(METADATA_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > METADATA_BYTES {
        return Err(HarnessError::Limits);
    }
    String::from_utf8(bytes).map_err(|_| HarnessError::Contract)
}
fn root_values(root: &Path) -> Result<Vec<String>, HarnessError> {
    ROOT_FILES
        .iter()
        .map(|name| read_metadata(&root.join(name)))
        .collect()
}
fn require_controllers(values: &[String]) -> Result<(), HarnessError> {
    if values.len() != ROOT_FILES.len()
        || !values[..2].iter().all(|s| {
            ["cpu", "memory", "pids"]
                .iter()
                .all(|required| s.split_whitespace().any(|v| v == *required))
        })
    {
        return Err(HarnessError::Contract);
    }
    Ok(())
}
fn cgroup_path(value: &str) -> Result<PathBuf, HarnessError> {
    let path = Path::new(value);
    if !plain_absolute(path) || value.contains(['\n', '\r', '\0']) || path == Path::new("/") {
        return Err(HarnessError::Contract);
    }
    Ok(Path::new("/sys/fs/cgroup")
        .join(path.strip_prefix("/").map_err(|_| HarnessError::Contract)?))
}
fn properties(text: &str) -> Result<BTreeMap<String, String>, HarnessError> {
    let mut values = BTreeMap::new();
    for line in text.lines() {
        let (key, value) = line.split_once('=').ok_or(HarnessError::Contract)?;
        if key.is_empty()
            || !key.bytes().all(|c| c.is_ascii_alphanumeric())
            || value.contains(['\0', '\r'])
            || values.insert(key.to_owned(), value.to_owned()).is_some()
        {
            return Err(HarnessError::Contract);
        }
    }
    Ok(values)
}
fn key_values(text: &str) -> Result<BTreeMap<String, u64>, HarnessError> {
    let mut out = BTreeMap::new();
    for line in text.lines() {
        let mut fields = line.split_whitespace();
        let key = fields.next().ok_or(HarnessError::Contract)?;
        let value = fields
            .next()
            .ok_or(HarnessError::Contract)?
            .parse()
            .map_err(|_| HarnessError::Contract)?;
        if fields.next().is_some() || out.insert(key.to_owned(), value).is_some() {
            return Err(HarnessError::Contract);
        }
    }
    Ok(out)
}
fn number(text: &str) -> Result<u64, HarnessError> {
    let value = text.trim();
    if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
        return Err(HarnessError::Contract);
    }
    value.parse().map_err(|_| HarnessError::Contract)
}
fn duration_us(text: &str) -> Result<u64, HarnessError> {
    let mut sum = 0_u64;
    let mut any = false;
    for item in text.split_whitespace() {
        let split = item
            .find(|c: char| !c.is_ascii_digit())
            .ok_or(HarnessError::Contract)?;
        let (value, unit) = item.split_at(split);
        let factor = match unit {
            "us" => 1,
            "ms" => 1_000,
            "s" => 1_000_000,
            "min" => 60_000_000,
            "h" => 3_600_000_000,
            _ => return Err(HarnessError::Contract),
        };
        sum = sum
            .checked_add(
                number(value)?
                    .checked_mul(factor)
                    .ok_or(HarnessError::Limits)?,
            )
            .ok_or(HarnessError::Limits)?;
        any = true;
    }
    if !any {
        return Err(HarnessError::Contract);
    }
    Ok(sum)
}
fn verify_lifecycle(
    values: &BTreeMap<String, String>,
    process: &ProcessSpec,
) -> Result<(), HarnessError> {
    let scope = process.containment.as_ref().ok_or(HarnessError::Contract)?;
    for (name, expected) in [
        ("RuntimeMaxUSec", process.limits.wall_ms),
        ("TimeoutStopUSec", scope.cancellation_grace_ms),
    ] {
        let actual = duration_us(values.get(name).ok_or(HarnessError::Contract)?)?;
        if actual != expected.checked_mul(1000).ok_or(HarnessError::Limits)? {
            return Err(HarnessError::TargetChanged);
        }
    }
    for (name, expected) in [
        ("SendSIGKILL", "yes"),
        ("KillMode", "control-group"),
        ("KillSignal", "15"),
        ("FinalKillSignal", "9"),
    ] {
        if values.get(name).map(String::as_str) != Some(expected) {
            return Err(HarnessError::TargetChanged);
        }
    }
    Ok(())
}
fn read_controls(group: &Path) -> Result<ScopeControls, HarnessError> {
    let cpu = read_metadata(&group.join("cpu.max"))?;
    let fields: Vec<_> = cpu.split_whitespace().collect();
    if fields.len() != 2 {
        return Err(HarnessError::Contract);
    }
    Ok(ScopeControls {
        cpu_quota_us: number(fields[0])?,
        cpu_period_us: number(fields[1])?,
        memory_bytes: number(&read_metadata(&group.join("memory.max"))?)?,
        swap_bytes: number(&read_metadata(&group.join("memory.swap.max"))?)?,
        tasks_max: number(&read_metadata(&group.join("pids.max"))?)?,
    })
}
fn optional_counter(path: &Path, previous: Option<u64>) -> Result<Option<u64>, HarnessError> {
    match read_metadata(path) {
        Ok(text) => {
            let n = number(&text)?;
            if previous.is_some_and(|old| n < old) {
                return Err(HarnessError::TargetChanged);
            }
            Ok(Some(n))
        }
        Err(HarnessError::Io(io::ErrorKind::NotFound)) => Ok(previous),
        Err(e) => Err(e),
    }
}
fn verify_membership(process: &ProcessIdentity, group: &Path) -> Result<(), HarnessError> {
    let root = PathBuf::from(format!("/proc/{}", process.pid));
    let first = start_ticks(&read_metadata(&root.join("stat"))?)?;
    let membership = read_metadata(&root.join("cgroup"))?;
    let entries: Vec<_> = membership.lines().collect();
    if entries.len() != 1
        || cgroup_path(
            entries[0]
                .strip_prefix("0::")
                .ok_or(HarnessError::Contract)?,
        )? != group
        || first != process.start_ticks
        || start_ticks(&read_metadata(&root.join("stat"))?)? != first
    {
        return Err(HarnessError::TargetChanged);
    }
    Ok(())
}
fn start_ticks(stat: &str) -> Result<u64, HarnessError> {
    let (_, tail) = stat.rsplit_once(") ").ok_or(HarnessError::Contract)?;
    number(
        tail.split_whitespace()
            .nth(19)
            .ok_or(HarnessError::Contract)?,
    )
}

struct HelperOutput {
    status: ExitStatus,
    output: String,
}
fn run_helper(
    mut command: Command,
    outer_deadline: Instant,
    pending: &mut Vec<PendingHelper>,
) -> Result<HelperOutput, HarnessError> {
    if Instant::now() >= outer_deadline {
        return Err(HarnessError::Io(io::ErrorKind::TimedOut));
    }
    let (mut stdout, output) = UnixStream::pair()?;
    let (mut stderr, error) = UnixStream::pair()?;
    stdout.set_nonblocking(true)?;
    stderr.set_nonblocking(true)?;
    command
        .stdout(Stdio::from(OwnedFd::from(output)))
        .stderr(Stdio::from(OwnedFd::from(error)));
    let deadline = outer_deadline.min(
        Instant::now()
            .checked_add(HELPER_TIME)
            .ok_or(HarnessError::Limits)?,
    );
    let mut child = command.spawn()?;
    // Command retains its configured parent-side Stdio handles after spawn.
    // Releasing them is necessary before EOF can establish helper completion.
    drop(command);
    let identity = ScopeHelperIdentity {
        pid: child.id(),
        start_ticks: read_metadata(&PathBuf::from(format!("/proc/{}/stat", child.id())))
            .ok()
            .and_then(|text| start_ticks(&text).ok()),
    };
    let mut captured = Vec::new();
    let mut total = 0_usize;
    let mut closed = [false; 2];
    let mut status = None;
    let result = 'outer: loop {
        for (i, stream) in [&mut stdout, &mut stderr].into_iter().enumerate() {
            if closed[i] {
                continue;
            }
            let mut chunk = [0_u8; 1024];
            match stream.read(&mut chunk) {
                Ok(0) => closed[i] = true,
                Ok(n) => {
                    let retain = n.min(HELPER_BYTES.saturating_sub(total));
                    total += n;
                    if i == 0 {
                        captured.extend_from_slice(&chunk[..retain]);
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
                Err(e) => break 'outer Err(HarnessError::Io(e.kind())),
            }
        }
        if total > HELPER_BYTES {
            break Err(HarnessError::Limits);
        }
        match child.try_wait() {
            Ok(s) => {
                if s.is_some() {
                    status = s;
                }
            }
            Err(e) => break Err(HarnessError::Io(e.kind())),
        }
        if closed == [true, true]
            && let Some(status) = status
        {
            break String::from_utf8(captured)
                .map(|output| HelperOutput { status, output })
                .map_err(|_| HarnessError::Contract);
        }
        if Instant::now() >= deadline {
            break Err(HarnessError::Io(io::ErrorKind::TimedOut));
        }
        thread::sleep(POLL.min(deadline.saturating_duration_since(Instant::now())));
    };
    if result.is_err() {
        let _ = child.kill();
        loop {
            match child.try_wait() {
                Ok(Some(_)) => break,
                Ok(None) if Instant::now() < outer_deadline => thread::sleep(
                    POLL.min(outer_deadline.saturating_duration_since(Instant::now())),
                ),
                _ => {
                    pending.push(PendingHelper { child, identity });
                    break;
                }
            }
        }
    }
    result
}

#[cfg(test)]
#[path = "owned_scope_tests.rs"]
mod tests;
