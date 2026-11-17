//! Explicitly authorized cgroup-v2 delegation, never discovered or enabled.
//!
//! Controller presence, installed readback and empirical enforcement are separate
//! facts. This adapter changes only a freshly owned child. Drop never signals,
//! deletes, refunds or fabricates closure. The caller retains cleanup ownership.
use super::ExecutionError;
use cortex_core::program::supervision::ScopeControls;
use rustix::fs::{Mode, OFlags, ResolveFlags};
use std::{
    fs::{self, File},
    io::{Read, Seek, SeekFrom, Write},
    os::{fd::AsFd, unix::fs::MetadataExt},
    path::{Path, PathBuf},
    process::Child,
};

/// Operator assertion selecting one already delegated root. It is not an
/// authenticated principal or proof that kernel controls have been enforced.
pub struct DelegationAuthorization {
    /// Exact canonical absolute root; never inferred from the desktop scope.
    pub root: PathBuf,
    /// Expected filesystem device.
    pub device: u64,
    /// Expected directory inode.
    pub inode: u64,
    /// Exact current boot, supplied explicitly.
    pub boot: String,
}
/// Held authorized root. No ancestor controller is enabled by this adapter.
pub struct Delegation {
    root: File,
    path: PathBuf,
    device: u64,
    inode: u64,
    boot: String,
}
fn open(root: impl AsFd, name: &Path, write: bool) -> Result<File, ExecutionError> {
    Ok(File::from(rustix::fs::openat2(
        root,
        name,
        (if write {
            OFlags::WRONLY
        } else {
            OFlags::RDONLY
        }) | OFlags::CLOEXEC
            | OFlags::NOFOLLOW,
        Mode::empty(),
        ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_MAGICLINKS,
    )?))
}
fn termination_control(dir: &File) -> Result<File, ExecutionError> {
    open(dir, Path::new("cgroup.kill"), true)
}
fn text(file: &File) -> Result<String, ExecutionError> {
    let mut f = file;
    f.seek(SeekFrom::Start(0))?;
    let mut result = String::new();
    Read::take(&mut f, 16385).read_to_string(&mut result)?;
    if result.len() > 16384 {
        return Err(ExecutionError::Policy);
    }
    Ok(result)
}
fn write(file: &File, value: &str) -> Result<(), ExecutionError> {
    let mut f = file;
    f.seek(SeekFrom::Start(0))?;
    f.write_all(value.as_bytes())?;
    Ok(())
}
pub(crate) fn validate_controls(c: &ScopeControls) -> Result<(), ExecutionError> {
    if c.cpu_quota_us < 1000
        || !(1000..=1_000_000).contains(&c.cpu_period_us)
        || c.memory_bytes == 0
        || c.tasks_max == 0
    {
        return Err(ExecutionError::Policy);
    }
    Ok(())
}
fn bound_relative(root: &Path, control_group: &str) -> Result<PathBuf, ExecutionError> {
    let group = Path::new(control_group);
    if !group.is_absolute()
        || group.components().any(|c| {
            !matches!(
                c,
                std::path::Component::RootDir | std::path::Component::Normal(_)
            )
        })
    {
        return Err(ExecutionError::Identity);
    }
    let full = Path::new("/sys/fs/cgroup").join(
        group
            .strip_prefix("/")
            .map_err(|_| ExecutionError::Identity)?,
    );
    let relative = full
        .strip_prefix(root)
        .map_err(|_| ExecutionError::Identity)?;
    if relative.as_os_str().is_empty() {
        return Err(ExecutionError::Identity);
    }
    Ok(relative.into())
}
fn controllers(available: &str, enabled: &str) -> Result<(), ExecutionError> {
    for required in ["cpu", "memory", "pids"] {
        if !available.split_whitespace().any(|v| v == required)
            || !enabled.split_whitespace().any(|v| v == required)
        {
            return Err(ExecutionError::Delegation);
        }
    }
    Ok(())
}
impl Delegation {
    /// Read-only verification. None refuses before opening any host path. An
    /// assertion does not authorize this method's caller to mutate host controls.
    pub fn inspect(authorization: Option<DelegationAuthorization>) -> Result<Self, ExecutionError> {
        let a = authorization.ok_or(ExecutionError::Delegation)?;
        if !a.root.is_absolute()
            || fs::canonicalize(&a.root)? != a.root
            || fs::read_to_string("/proc/sys/kernel/random/boot_id")?.trim() != a.boot
        {
            return Err(ExecutionError::Identity);
        }
        let root = File::from(rustix::fs::open(
            &a.root,
            OFlags::DIRECTORY | OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
            Mode::empty(),
        )?);
        let meta = root.metadata()?;
        // CGROUP2_SUPER_MAGIC is the kernel UAPI filesystem identity.
        if rustix::fs::fstatfs(&root)?.f_type as u64 != 0x63677270
            || meta.dev() != a.device
            || meta.ino() != a.inode
            || meta.uid() != rustix::process::getuid().as_raw()
        {
            return Err(ExecutionError::Identity);
        }
        if text(&open(&root, Path::new("cgroup.type"), false)?)?.trim() != "domain" {
            return Err(ExecutionError::Delegation);
        }
        controllers(
            &text(&open(&root, Path::new("cgroup.controllers"), false)?)?,
            &text(&open(&root, Path::new("cgroup.subtree_control"), false)?)?,
        )?;
        Ok(Self {
            root,
            path: a.root,
            device: a.device,
            inode: a.inode,
            boot: a.boot,
        })
    }
    /// Pin an already manager-created exact incarnation beneath the explicitly
    /// authorized root. This does not create, configure, attach or signal work.
    /// A held descriptor, rather than a manager lookup name, selects later stop.
    pub(crate) fn inspect_bound_group(
        &self,
        identity: &cortex_core::program::domain::DomainIdentity,
        controls: &ScopeControls,
    ) -> Result<OwnedGroup, ExecutionError> {
        self.check()?;
        if identity.boot != self.boot {
            return Err(ExecutionError::Identity);
        }
        let relative = bound_relative(&self.path, &identity.control_group)?;
        let dir = File::from(rustix::fs::openat2(
            &self.root,
            &relative,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
            ResolveFlags::BENEATH
                | ResolveFlags::NO_SYMLINKS
                | ResolveFlags::NO_MAGICLINKS
                | ResolveFlags::NO_XDEV,
        )?);
        let m = dir.metadata()?;
        if m.dev() != identity.cgroup_device
            || m.ino() != identity.cgroup_inode
            || m.uid() != rustix::process::getuid().as_raw()
            || rustix::fs::fstatfs(&dir)?.f_type as u64 != 0x63677270
            || text(&open(&dir, Path::new("cgroup.type"), false)?)?.trim() != "domain"
        {
            return Err(ExecutionError::Identity);
        }
        let name = relative
            .to_str()
            .ok_or(ExecutionError::Identity)?
            .to_owned();
        // Read-only preparation of targeting authority: open, never write.
        let termination = termination_control(&dir)?;
        let group = OwnedGroup {
            dir,
            parent: self.root.try_clone()?,
            name,
            path: self.path.clone(),
            device: m.dev(),
            inode: m.ino(),
            boot: self.boot.clone(),
            controls: Some(controls.clone()),
            attached: None,
            termination: Some(termination),
        };
        group.check_controls()?;
        Ok(group)
    }
    /// Reopen only the exact recorded group for read-only recovery. This never
    /// creates a group, reconstructs a live release channel, attaches or signals.
    pub fn inspect_recovery(
        &self,
        identity: &cortex_core::program::domain::DomainIdentity,
    ) -> RecoveryObservation {
        if self.check().is_err() || identity.boot != self.boot {
            return RecoveryObservation::Unverifiable;
        }
        let path = Path::new(&identity.control_group);
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            return RecoveryObservation::Unverifiable;
        };
        if path.parent() != Some(self.path.as_path()) || !name.starts_with("cortex-") {
            return RecoveryObservation::Unverifiable;
        }
        let dir = match rustix::fs::openat2(
            &self.root,
            name,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
            ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS,
        ) {
            Ok(fd) => File::from(fd),
            // A missing lookup path can be a renamed surviving directory.
            // Only a held descriptor with nlink=0 supports Gone.
            Err(rustix::io::Errno::NOENT) => return RecoveryObservation::Unverifiable,
            Err(_) => return RecoveryObservation::Unverifiable,
        };
        let Ok(m) = dir.metadata() else {
            return RecoveryObservation::Unverifiable;
        };
        if (m.dev(), m.ino()) != (identity.cgroup_device, identity.cgroup_inode) {
            return RecoveryObservation::Replaced;
        }
        let parent = match self.root.try_clone() {
            Ok(f) => f,
            Err(_) => return RecoveryObservation::Unverifiable,
        };
        let group = OwnedGroup {
            dir,
            parent,
            name: name.into(),
            path: self.path.clone(),
            device: m.dev(),
            inode: m.ino(),
            boot: self.boot.clone(),
            controls: None,
            attached: None,
            termination: None,
        };
        group.recover()
    }

    pub(crate) fn check(&self) -> Result<(), ExecutionError> {
        let m = self.root.metadata()?;
        if m.dev() != self.device
            || m.ino() != self.inode
            || m.nlink() == 0
            || fs::read_to_string("/proc/sys/kernel/random/boot_id")?.trim() != self.boot
        {
            return Err(ExecutionError::Identity);
        }
        controllers(
            &text(&open(&self.root, Path::new("cgroup.controllers"), false)?)?,
            &text(&open(
                &self.root,
                Path::new("cgroup.subtree_control"),
                false,
            )?)?,
        )?;
        Ok(())
    }
    /// Create one exclusive child. Partial initialization leaves an explicitly
    /// owned directory for observation; no unowned/shared target is adopted.
    /// Requires separate operator authority to mutate this delegated scope.
    pub fn create(&self, name: &str) -> Result<OwnedGroup, ExecutionError> {
        self.check()?;
        if !crate::domain_bootstrap::token(name) {
            return Err(ExecutionError::Policy);
        }
        let name = format!("cortex-{name}");
        rustix::fs::mkdirat(&self.root, name.as_str(), Mode::RWXU)?;
        let dir = File::from(rustix::fs::openat2(
            &self.root,
            name.as_str(),
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
            ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS,
        )?);
        let m = dir.metadata()?;
        Ok(OwnedGroup {
            dir,
            parent: self.root.try_clone()?,
            name,
            path: self.path.clone(),
            device: m.dev(),
            inode: m.ino(),
            boot: self.boot.clone(),
            controls: None,
            attached: None,
            termination: None,
        })
    }
}
/// Exact owned group; held descriptors select all control and kill operations.
/// This is a runtime resource, never a second canonical state ledger.
pub struct OwnedGroup {
    dir: File,
    parent: File,
    name: String,
    path: PathBuf,
    device: u64,
    inode: u64,
    boot: String,
    controls: Option<ScopeControls>,
    attached: Option<(u32, u64)>,
    // Opened before readiness; never forwarded to bootstrap or payload.
    termination: Option<File>,
}
/// Finite point-in-time group observation. No field alone settles reservations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupObservation {
    /// Actual recursive populated bit, not inferred from direct-child exit.
    pub populated: bool,
    /// Current direct members.
    pub members: Vec<u32>,
    /// Cumulative CPU microseconds, if the kernel exposes the counter.
    pub cpu_micros: Option<u64>,
    /// Charged memory peak in bytes, if available.
    pub memory_peak: Option<u64>,
}
/// Recovery never recreates groups, adopts a numeric PID, or repeats a release.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecoveryObservation {
    /// Matching held incarnation with current kernel population.
    Present(GroupObservation),
    /// Original directory no longer exists; historical effects remain unknown.
    Gone,
    /// Name now resolves to another inode; never target it.
    Replaced,
    /// Readback failed; no permission to signal or refund.
    Unverifiable,
}
impl OwnedGroup {
    fn check(&self) -> Result<(), ExecutionError> {
        let held = self.dir.metadata()?;
        if held.dev() != self.device
            || held.ino() != self.inode
            || held.nlink() == 0
            || fs::read_to_string("/proc/sys/kernel/random/boot_id")?.trim() != self.boot
        {
            return Err(ExecutionError::Identity);
        }
        let current = File::from(rustix::fs::openat2(
            &self.parent,
            self.name.as_str(),
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
            ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS,
        )?);
        let m = current.metadata()?;
        if m.dev() != self.device || m.ino() != self.inode {
            return Err(ExecutionError::Identity);
        }
        Ok(())
    }
    /// Exact directory device/inode, suitable for canonical DomainIdentity.
    pub fn identity(&self) -> (u64, u64) {
        (self.device, self.inode)
    }
    /// Explicit selected path; operations continue to use held descriptors.
    pub fn path(&self) -> PathBuf {
        self.path.join(&self.name)
    }
    /// Install and read back all mandatory limits before attaching a child.
    /// Partial failure never becomes a configured or releasable group.
    pub fn configure(&mut self, controls: &ScopeControls) -> Result<(), ExecutionError> {
        validate_controls(controls)?;
        self.check()?;
        if self.attached.is_some() || self.observe()?.populated {
            return Err(ExecutionError::Identity);
        }
        self.controls = None;
        install_controls(&mut GroupIo(&self.dir), controls)?;
        self.termination = Some(termination_control(&self.dir)?);
        self.controls = Some(controls.clone());
        Ok(())
    }
    /// Read back exact limits; an available controller is not installed policy.
    pub fn check_controls(&self) -> Result<&ScopeControls, ExecutionError> {
        self.check()?;
        let c = self.controls.as_ref().ok_or(ExecutionError::Delegation)?;
        for (name, value) in control_values(c) {
            if text(&open(&self.dir, Path::new(name), false)?)?.trim() != value {
                return Err(ExecutionError::Identity);
            }
        }
        Ok(c)
    }
    pub(crate) fn attach(&mut self, child: &mut Child) -> Result<(), ExecutionError> {
        self.check_controls()?;
        if self.attached.is_some() || child.try_wait()?.is_some() {
            return Err(ExecutionError::Identity);
        }
        // Exclusive unreaped Child ownership is retained through attach. Bootstrap
        // is blocked on its setup frame and cannot fork a payload at this point.
        let pid = child.id();
        let ticks = crate::domain_bootstrap::ticks(pid)?;
        write(
            &open(&self.dir, Path::new("cgroup.procs"), true)?,
            &pid.to_string(),
        )?;
        if child.try_wait()?.is_some()
            || crate::domain_bootstrap::ticks(pid)? != ticks
            || self.observe()?.members != [pid]
        {
            return Err(ExecutionError::Identity);
        }
        self.attached = Some((pid, ticks));
        Ok(())
    }
    /// Read bounded kernel facts. Recursive population is authoritative for this
    /// observation; direct membership alone cannot prove descendant quiescence.
    pub fn observe(&self) -> Result<GroupObservation, ExecutionError> {
        self.check()?;
        let events = text(&open(&self.dir, Path::new("cgroup.events"), false)?)?;
        let populated = parse_populated(&events)?;
        let members = text(&open(&self.dir, Path::new("cgroup.procs"), false)?)?
            .split_whitespace()
            .map(|s| s.parse::<u32>().map_err(|_| ExecutionError::Identity))
            .collect::<Result<Vec<_>, _>>()?;
        if members.len() > 4096 {
            return Err(ExecutionError::Policy);
        }
        let cpu = text(&open(&self.dir, Path::new("cpu.stat"), false)?)?;
        let cpu_micros = cpu
            .lines()
            .find_map(|l| l.strip_prefix("usage_usec "))
            .and_then(|n| n.parse().ok());
        let memory_peak = open(&self.dir, Path::new("memory.peak"), false)
            .ok()
            .and_then(|f| text(&f).ok())
            .and_then(|s| s.trim().parse().ok());
        Ok(GroupObservation {
            populated,
            members,
            cpu_micros,
            memory_peak,
        })
    }
    /// Scoped kernel subtree termination. Caller must first durably record
    /// cancellation through the canonical owner; this method grants no authority.
    /// Repeated requests do not target a replaced group or recycled PID.
    pub fn request_termination(&self) -> Result<(), ExecutionError> {
        self.check_controls()?;
        let termination = self
            .termination
            .as_ref()
            .ok_or(ExecutionError::Delegation)?;
        write(termination, "1")?;
        Ok(())
    }
    /// Read-only reinspection. No signal, recreation, replay or settlement.
    pub fn recover(&self) -> RecoveryObservation {
        match self.dir.metadata() {
            Ok(m) if m.nlink() == 0 => RecoveryObservation::Gone,
            Err(_) => RecoveryObservation::Unverifiable,
            _ => match self.check() {
                Ok(()) => self
                    .observe()
                    .map(RecoveryObservation::Present)
                    .unwrap_or(RecoveryObservation::Unverifiable),
                Err(ExecutionError::Identity) => RecoveryObservation::Replaced,
                Err(_) => RecoveryObservation::Unverifiable,
            },
        }
    }
}
fn control_values(c: &ScopeControls) -> [(&'static str, String); 4] {
    [
        ("cpu.max", format!("{} {}", c.cpu_quota_us, c.cpu_period_us)),
        ("memory.max", c.memory_bytes.to_string()),
        ("memory.swap.max", c.swap_bytes.to_string()),
        ("pids.max", c.tasks_max.to_string()),
    ]
}
fn parse_populated(s: &str) -> Result<bool, ExecutionError> {
    let values = s
        .lines()
        .filter_map(|l| l.strip_prefix("populated "))
        .collect::<Vec<_>>();
    match values.as_slice() {
        ["0"] => Ok(false),
        ["1"] => Ok(true),
        _ => Err(ExecutionError::Identity),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn renamed_group_path_absence_is_not_physical_disappearance() {
        // SOURCE_SIMULATION: ordinary owned directories, no kernel cgroup.
        use std::os::unix::fs::PermissionsExt;
        let root_path =
            std::env::temp_dir().join(format!("cortex-recovery-rename-{}", std::process::id()));
        fs::create_dir(&root_path).unwrap();
        fs::set_permissions(&root_path, fs::Permissions::from_mode(0o700)).unwrap();
        fs::write(root_path.join("cgroup.controllers"), "cpu memory pids").unwrap();
        fs::write(root_path.join("cgroup.subtree_control"), "cpu memory pids").unwrap();
        fs::create_dir(root_path.join("cortex-old")).unwrap();
        let root = File::open(&root_path).unwrap();
        let m = root.metadata().unwrap();
        let child = fs::metadata(root_path.join("cortex-old")).unwrap();
        let boot = fs::read_to_string("/proc/sys/kernel/random/boot_id")
            .unwrap()
            .trim()
            .to_owned();
        let scope = Delegation {
            root,
            path: root_path.clone(),
            device: m.dev(),
            inode: m.ino(),
            boot: boot.clone(),
        };
        let identity = cortex_core::program::domain::DomainIdentity {
            pool: cortex_core::program::domain::PoolIdentity {
                physical: "fixture".into(),
                incarnation: "one".into(),
                host: "fixture".into(),
                boot: boot.clone(),
            },
            request: cortex_core::program::OperationId::new("rename").unwrap(),
            manager: "fixture".into(),
            invocation: "one".into(),
            control_group: root_path.join("cortex-old").to_str().unwrap().into(),
            cgroup_device: child.dev(),
            cgroup_inode: child.ino(),
            boot,
        };
        fs::rename(root_path.join("cortex-old"), root_path.join("renamed")).unwrap();
        assert!(matches!(
            scope.inspect_recovery(&identity),
            RecoveryObservation::Unverifiable
        ));
        assert_eq!(
            fs::metadata(root_path.join("renamed")).unwrap().ino(),
            identity.cgroup_inode
        );
        fs::remove_dir_all(root_path).unwrap();
    }
    #[test]
    fn termination_must_be_pinned_writable_before_readiness_and_not_retargeted() {
        // SOURCE_SIMULATION: ordinary files, no actual cgroup or process kill.
        use std::os::unix::fs::PermissionsExt;
        let path =
            std::env::temp_dir().join(format!("cortex-termination-pin-{}", std::process::id()));
        fs::create_dir(&path).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        let dir = File::open(&path).unwrap();
        assert!(termination_control(&dir).is_err());
        fs::write(path.join("cgroup.kill"), "0").unwrap();
        fs::set_permissions(path.join("cgroup.kill"), fs::Permissions::from_mode(0o400)).unwrap();
        assert!(
            termination_control(&dir).is_err(),
            "non-root fixture requires actual write permission"
        );
        fs::set_permissions(path.join("cgroup.kill"), fs::Permissions::from_mode(0o600)).unwrap();
        let pinned = termination_control(&dir).unwrap();
        fs::rename(path.join("cgroup.kill"), path.join("original")).unwrap();
        fs::write(path.join("cgroup.kill"), "neighbor").unwrap();
        write(&pinned, "1").unwrap();
        assert_eq!(fs::read_to_string(path.join("original")).unwrap(), "1");
        assert_eq!(
            fs::read_to_string(path.join("cgroup.kill")).unwrap(),
            "neighbor"
        );
        fs::remove_dir_all(path).unwrap();
    }
    #[test]
    fn managed_group_pin_cannot_select_root_neighbor_or_traversal() {
        let root = Path::new("/sys/fs/cgroup/owned");
        assert_eq!(
            bound_relative(root, "/owned/child.service"),
            Ok(PathBuf::from("child.service"))
        );
        assert_eq!(
            bound_relative(root, "/owned/nested/child.service"),
            Ok(PathBuf::from("nested/child.service"))
        );
        for wrong in [
            "/owned",
            "/neighbor/job",
            "/owned/../neighbor",
            "relative",
            "/",
        ] {
            assert_eq!(bound_relative(root, wrong), Err(ExecutionError::Identity));
        }
    }
    #[test]
    fn absent_authorization_never_discovers_a_scope() {
        assert!(matches!(
            Delegation::inspect(None),
            Err(ExecutionError::Delegation)
        ));
    }
    #[test]
    fn availability_is_not_delegation() {
        assert_eq!(
            controllers("cpu memory pids", "memory pids"),
            Err(ExecutionError::Delegation)
        );
        assert_eq!(controllers("cpu memory pids", "cpu memory pids"), Ok(()));
        assert_eq!(
            controllers("cpu memory", "cpu memory pids"),
            Err(ExecutionError::Delegation)
        );
    }
    #[test]
    fn recursive_population_is_required_not_direct_child_exit() {
        for text in ["", "populated 2", "populated 0\npopulated 1"] {
            assert!(parse_populated(text).is_err());
        }
        assert_eq!(parse_populated("populated 1\nfrozen 0\n"), Ok(true));
        assert_eq!(parse_populated("populated 0\nfrozen 0\n"), Ok(false));
    }
    #[test]
    fn limits_are_explicit_and_swap_zero_remains_zero() {
        let c = ScopeControls {
            cpu_quota_us: 50000,
            cpu_period_us: 100000,
            memory_bytes: 134217728,
            swap_bytes: 0,
            tasks_max: 16,
        };
        assert_eq!(control_values(&c)[2], ("memory.swap.max", "0".into()));
        let mut invalid = c;
        invalid.memory_bytes = 0;
        assert!(validate_controls(&invalid).is_err());
    }
}

trait ControlIo {
    fn read(&mut self, name: &str) -> Result<String, ExecutionError>;
    fn write(&mut self, name: &str, value: &str) -> Result<(), ExecutionError>;
}
struct GroupIo<'a>(&'a File);
impl ControlIo for GroupIo<'_> {
    fn read(&mut self, name: &str) -> Result<String, ExecutionError> {
        text(&open(self.0, Path::new(name), false)?)
    }
    fn write(&mut self, name: &str, value: &str) -> Result<(), ExecutionError> {
        write(&open(self.0, Path::new(name), true)?, value)
    }
}
fn install_controls(io: &mut impl ControlIo, c: &ScopeControls) -> Result<(), ExecutionError> {
    validate_controls(c)?;
    for (name, value) in control_values(c) {
        io.write(name, &value)?;
        if io.read(name)?.trim() != value {
            return Err(ExecutionError::Identity);
        }
    }
    Ok(())
}
#[cfg(test)]
mod fault_tests {
    use super::*;
    #[derive(Default)]
    struct Fake {
        values: std::collections::BTreeMap<String, String>,
        steps: usize,
        fail: Option<usize>,
        changed: bool,
    }
    impl ControlIo for Fake {
        fn read(&mut self, name: &str) -> Result<String, ExecutionError> {
            self.steps += 1;
            if self.fail == Some(self.steps) {
                return Err(ExecutionError::Io(std::io::ErrorKind::Interrupted));
            }
            if self.changed {
                return Ok("max".into());
            }
            Ok(self.values[name].clone())
        }
        fn write(&mut self, name: &str, value: &str) -> Result<(), ExecutionError> {
            self.steps += 1;
            if self.fail == Some(self.steps) {
                return Err(ExecutionError::Io(std::io::ErrorKind::Interrupted));
            }
            self.values.insert(name.into(), value.into());
            Ok(())
        }
    }
    #[test]
    fn every_interrupted_install_or_missing_readback_refuses() {
        let c = ScopeControls {
            cpu_quota_us: 50000,
            cpu_period_us: 100000,
            memory_bytes: 134217728,
            swap_bytes: 0,
            tasks_max: 8,
        };
        for step in 1..=8 {
            let mut io = Fake {
                fail: Some(step),
                ..Default::default()
            };
            assert_eq!(
                install_controls(&mut io, &c),
                Err(ExecutionError::Io(std::io::ErrorKind::Interrupted))
            );
            assert_eq!(io.steps, step);
        }
        let mut changed = Fake {
            changed: true,
            ..Default::default()
        };
        assert_eq!(
            install_controls(&mut changed, &c),
            Err(ExecutionError::Identity)
        );
        let mut valid = Fake::default();
        assert_eq!(install_controls(&mut valid, &c), Ok(()));
        assert_eq!(valid.steps, 8);
        assert_eq!(valid.values["memory.swap.max"], "0");
    }
}

pub(crate) struct BootstrapGroup {
    pub path: PathBuf,
    pub device: u64,
    pub inode: u64,
    pub boot: String,
    pub controls: ScopeControls,
}
fn member_path(path: &Path, membership: &str) -> Result<(), ExecutionError> {
    let lines = membership.lines().collect::<Vec<_>>();
    let [line] = lines.as_slice() else {
        return Err(ExecutionError::Identity);
    };
    let relative = line.strip_prefix("0::/").ok_or(ExecutionError::Identity)?;
    if relative.is_empty()
        || Path::new(relative)
            .components()
            .any(|c| !matches!(c, std::path::Component::Normal(_)))
        || Path::new("/sys/fs/cgroup").join(relative) != path
    {
        return Err(ExecutionError::Identity);
    }
    Ok(())
}
/// Read-only bootstrap check before namespaces/payload. No supplied pathname or
/// parent frame alone establishes containment. No descriptor survives this call.
pub(crate) fn verify_bootstrap_group(expected: &BootstrapGroup) -> Result<(), ExecutionError> {
    validate_controls(&expected.controls)?;
    if expected.controls.tasks_max > 8
        || expected.controls.swap_bytes != 0
        || !expected.path.is_absolute()
        || expected.path.as_os_str().len() > 4096
        || expected.path.components().any(|c| {
            !matches!(
                c,
                std::path::Component::RootDir | std::path::Component::Normal(_)
            )
        })
    {
        return Err(ExecutionError::Policy);
    }
    member_path(&expected.path, &text(&File::open("/proc/self/cgroup")?)?)?;
    if text(&File::open("/proc/sys/kernel/random/boot_id")?)?.trim() != expected.boot {
        return Err(ExecutionError::Identity);
    }
    let host = rustix::fs::open(
        "/",
        OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )?;
    let relative = expected
        .path
        .strip_prefix("/")
        .map_err(|_| ExecutionError::Policy)?;
    let dir = File::from(rustix::fs::openat2(
        &host,
        relative,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
        ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_MAGICLINKS,
    )?);
    let m = dir.metadata()?;
    if rustix::fs::fstatfs(&dir)?.f_type as u64 != 0x63677270
        || (m.dev(), m.ino()) != (expected.device, expected.inode)
        || text(&open(&dir, Path::new("cgroup.type"), false)?)?.trim() != "domain"
    {
        return Err(ExecutionError::Identity);
    }
    for (name, value) in control_values(&expected.controls) {
        if text(&open(&dir, Path::new(name), false)?)?.trim() != value {
            return Err(ExecutionError::Identity);
        }
    }
    let actual = text(&open(&dir, Path::new("cgroup.procs"), false)?)?;
    let members = actual.split_whitespace().collect::<Vec<_>>();
    let own = std::process::id().to_string();
    if members != [own.as_str()]
        || !parse_populated(&text(&open(&dir, Path::new("cgroup.events"), false)?)?)?
    {
        return Err(ExecutionError::Identity);
    }
    // Recheck membership after all reads. The parent also verifies the held
    // incarnation/controls inside canonical locked release; same-user host
    // administration remains a documented cooperating-operator assumption.
    member_path(&expected.path, &text(&File::open("/proc/self/cgroup")?)?)?;
    Ok(())
}
#[cfg(test)]
mod bootstrap_tests {
    use super::*;
    #[test]
    fn bootstrap_rejects_shared_root_ambiguous_or_changed_membership() {
        let expected = Path::new("/sys/fs/cgroup/owned/attempt");
        assert_eq!(
            member_path(
                expected,
                "0::/owned/attempt
"
            ),
            Ok(())
        );
        for invalid in [
            "0::/
",
            "1:cpu:/owned/attempt
",
            "0::/owned/neighbor
",
            "0::/owned/../attempt
",
            "0::/owned/attempt
0::/owned/attempt
",
        ] {
            assert_eq!(
                member_path(expected, invalid),
                Err(ExecutionError::Identity)
            );
        }
    }
}
