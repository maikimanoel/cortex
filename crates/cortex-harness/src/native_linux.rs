//! Native child-side controls for the fixed provider-free domain bootstrap.
//!
//! This is a restricted development profile, not an arbitrary-executable API.
//! Setup is irreversible in the bootstrap process and must never run in the
//! multi-threaded controller. Failure prevents readiness and the existing durable
//! release. Existing manager/cgroup controls remain separately required.

use cortex_core::program::executor::{Control, Support};
use rustix::{
    event::{PollFd, PollFlags, Timespec, poll},
    process::{Pid, PidfdFlags, pidfd_open},
};
use sha2::{Digest, Sha256};
use std::{
    fs, io,
    os::{fd::OwnedFd, unix::fs::MetadataExt},
    path::{Component, Path, PathBuf},
};

/// Exact caller-selected limits and owned empty mount point. No default budget.
/// Paths are intentionally absent from Debug and error messages.
#[derive(Clone, PartialEq, Eq)]
pub struct NativeIsolation {
    root: PathBuf,
    scratch_bytes: u64,
    scratch_inodes: u64,
    device: u64,
    inode: u64,
    uid: u32,
}
impl NativeIsolation {
    /// Pin an existing owner-only empty directory without installing controls.
    /// The cooperating operator must preserve its parent path until launch.
    pub fn inspect(root: &Path, scratch_bytes: u64, scratch_inodes: u64) -> io::Result<Self> {
        if scratch_bytes == 0
            || !scratch_bytes.is_multiple_of(4096)
            || scratch_inodes == 0
            || root.to_str().is_none()
            || !root.is_absolute()
            || root
                .components()
                .any(|c| !matches!(c, Component::RootDir | Component::Normal(_)))
            || fs::canonicalize(root)? != root
        {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let m = fs::symlink_metadata(root)?;
        if !m.is_dir()
            || m.mode() & 0o777 != 0o700
            || m.uid() != rustix::process::getuid().as_raw()
            || fs::read_dir(root)?.next().is_some()
        {
            return Err(io::ErrorKind::PermissionDenied.into());
        }
        Ok(Self {
            root: root.into(),
            scratch_bytes,
            scratch_inodes,
            device: m.dev(),
            inode: m.ino(),
            uid: m.uid(),
        })
    }
    pub(crate) fn check(&self) -> io::Result<()> {
        let current = Self::inspect(&self.root, self.scratch_bytes, self.scratch_inodes)?;
        if current != *self {
            return Err(io::ErrorKind::PermissionDenied.into());
        }
        Ok(())
    }
    /// Exact policy digest, including mount-point identity and explicit quotas.
    /// A digest is correspondence, not independent host qualification.
    pub fn digest(&self) -> String {
        let mut hash = Sha256::new();
        hash.update(b"cortex-native-isolation-v1\0");
        use std::os::unix::ffi::OsStrExt;
        hash.update(self.root.as_os_str().as_bytes());
        for n in [
            self.scratch_bytes,
            self.scratch_inodes,
            self.device,
            self.inode,
            self.uid.into(),
        ] {
            hash.update(n.to_be_bytes());
        }
        format!("{:x}", hash.finalize())
    }
    pub(crate) fn arguments(&self) -> Vec<String> {
        vec![
            "isolated-v1".into(),
            self.root.to_string_lossy().into_owned(),
            self.scratch_bytes.to_string(),
            self.scratch_inodes.to_string(),
            self.digest(),
        ]
    }
    /// Source availability before child setup. Never advertises live enforcement.
    /// Missing controls stay unknown; unsupported continuation cannot be selected.
    pub fn planned_support(control: &Control) -> Support {
        match control {
            Control::SessionContinuation => Support::Unsupported,
            _ => Support::Unknown,
        }
    }
}

/// Bootstrap-originated observation retained only after policy-bound hello and
/// manager/process correspondence. It is controlled-fixture evidence, never an
/// independently authenticated host qualification or a general transport claim.
#[derive(Clone)]
pub struct NativeReadiness {
    policy: NativeIsolation,
    boot: String,
}
impl NativeReadiness {
    pub(crate) fn observed(policy: &NativeIsolation, boot: &str) -> Self {
        Self {
            policy: policy.clone(),
            boot: boot.into(),
        }
    }
    /// Exact requested filesystem/scratch policy identity.
    pub fn policy_digest(&self) -> String {
        self.policy.digest()
    }
    /// Revalidate boot and immutable mount-point correspondence. This cannot
    /// prove continued child liveness; the prepared handle separately retains a
    /// pidfd and the existing manager/cgroup observation gate before release.
    pub fn check_context(&self) -> io::Result<()> {
        self.policy.check()?;
        if fs::read_to_string("/proc/sys/kernel/random/boot_id")?.trim() != self.boot {
            return Err(io::ErrorKind::InvalidData.into());
        }
        Ok(())
    }
    /// Project the three installed child controls into the existing Core
    /// capability vocabulary. All other controls remain absent and therefore
    /// held by Core negotiation. No caller-selected evidence class is upgraded.
    /// DiskBytes here covers this profile's only writable storage: scratch tmpfs.
    pub fn observations(
        &self,
        basis: &cortex_core::temporal::SourceReference,
    ) -> io::Result<Vec<cortex_core::program::executor::CapabilityObservation>> {
        use cortex_core::program::{executor::*, scheduling::Resource};
        self.check_context()?;
        Ok([
            Control::Filesystem,
            Control::Network,
            Control::Resource(Resource::DiskBytes),
        ]
        .into_iter()
        .map(|control| CapabilityObservation {
            control,
            kind: ObservationKind::ControlledFixture,
            support: Support::Supported,
            basis: basis.clone(),
        })
        .collect())
    }
}

/// Redacted installation phase. No private paths or syscall messages are exposed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IsolationPhase {
    /// Policy changed, malformed input, nonempty root or non-single-threaded caller.
    Policy,
    /// Unexpected descriptor could bypass confinement.
    Descriptors,
    /// User namespace or exact UID/GID mapping failed.
    UserNamespace,
    /// Isolated mount/network namespace creation failed.
    Namespaces,
    /// Mount propagation, bounded temporary root or scratch failed.
    Filesystem,
    /// Old-root removal or root transition failed.
    Root,
    /// Capability removal or no_new_privs failed.
    Privileges,
}
/// Finite installation failure. It never means the payload was released.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IsolationFailure {
    /// Earliest supported failing boundary.
    pub phase: IsolationPhase,
    /// Original OS error category without private content.
    pub kind: io::ErrorKind,
    /// Numeric OS error, if available.
    pub os_error: Option<i32>,
}
impl std::fmt::Display for IsolationFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "native isolation {:?}: {:?} os={:?}",
            self.phase, self.kind, self.os_error
        )
    }
}
impl std::error::Error for IsolationFailure {}
fn stage<T>(phase: IsolationPhase, r: io::Result<T>) -> Result<T, IsolationFailure> {
    r.map_err(|e| IsolationFailure {
        phase,
        kind: e.kind(),
        os_error: e.raw_os_error(),
    })
}
fn os<T>(r: Result<T, impl Into<io::Error>>) -> io::Result<T> {
    r.map_err(Into::into)
}

/// Check the complete descriptor table, excluding only stdio, the controller
/// channel and the enumeration descriptor itself. No broad numeric exemption.
pub(crate) fn descriptors(channel: i32) -> io::Result<()> {
    use std::os::fd::AsRawFd;
    // Stdio is not a filesystem/network exemption: all three must refer to the
    // null character device. The manager profile configures exactly this shape.
    let null = fs::metadata("/dev/null")?;
    for n in 0..=2 {
        let m = fs::metadata(format!("/proc/self/fd/{n}"))?;
        if m.mode() & 0o170000 != 0o020000 || m.rdev() != null.rdev() {
            return Err(io::ErrorKind::PermissionDenied.into());
        }
    }
    let fd = fs::File::open("/proc/self/fd")?;
    let mut dir = rustix::fs::Dir::new(fd)?;
    let own = dir.fd()?.as_raw_fd();
    let mut count = 0;
    while let Some(entry) = dir.read() {
        count += 1;
        if count > 256 {
            return Err(io::ErrorKind::InvalidData.into());
        }
        let entry = entry?;
        let bytes = entry.file_name().to_bytes();
        if bytes == b"." || bytes == b".." {
            continue;
        }
        let n: i32 = std::str::from_utf8(bytes)
            .ok()
            .and_then(|v| v.parse().ok())
            .ok_or(io::ErrorKind::InvalidData)?;
        if n > 2 && n != own && n != channel {
            return Err(io::ErrorKind::PermissionDenied.into());
        }
    }
    Ok(())
}

/// Irreversibly confine a dedicated single-threaded bootstrap. This function is
/// crate-private: controller callers cannot accidentally change their own host
/// view. No untrusted code runs here. The only retained non-stdio descriptor is
/// the bounded fixed-protocol controller socket, never an arbitrary payload FD.
pub(crate) fn install(policy: &NativeIsolation, channel: i32) -> Result<(), IsolationFailure> {
    use IsolationPhase::*;
    use nix::sched::{CloneFlags, unshare};
    use rustix::{mount::*, thread::*};
    stage(Policy, policy.check())?;
    stage(
        Policy,
        (|| {
            if fs::read_dir("/proc/self/task")?.count() != 1 {
                return Err(io::ErrorKind::PermissionDenied.into());
            }
            Ok(())
        })(),
    )?;
    stage(Descriptors, descriptors(channel))?;
    let uid = rustix::process::getuid().as_raw();
    let gid = rustix::process::getgid().as_raw();
    // Safe nix wrapper; no CLONE_FILES or shared descriptor-table operation.
    stage(UserNamespace, os(unshare(CloneFlags::CLONE_NEWUSER)))?;
    stage(UserNamespace, fs::write("/proc/self/setgroups", "deny"))?;
    stage(
        UserNamespace,
        fs::write("/proc/self/uid_map", format!("0 {uid} 1\n")),
    )?;
    stage(
        UserNamespace,
        fs::write("/proc/self/gid_map", format!("0 {gid} 1\n")),
    )?;
    let original_net = stage(Namespaces, fs::read_link("/proc/self/ns/net"))?;
    let original_mount = stage(Namespaces, fs::read_link("/proc/self/ns/mnt"))?;
    stage(
        Namespaces,
        os(unshare(
            CloneFlags::CLONE_NEWNS | CloneFlags::CLONE_NEWNET | CloneFlags::CLONE_NEWIPC,
        )),
    )?;
    if stage(Namespaces, fs::read_link("/proc/self/ns/net"))? == original_net
        || stage(Namespaces, fs::read_link("/proc/self/ns/mnt"))? == original_mount
    {
        return Err(IsolationFailure {
            phase: Namespaces,
            kind: io::ErrorKind::PermissionDenied,
            os_error: None,
        });
    }
    stage(
        Filesystem,
        os(mount_change(
            "/",
            MountPropagationFlags::PRIVATE | MountPropagationFlags::REC,
        )),
    )?;
    let flags = MountFlags::NOSUID | MountFlags::NODEV | MountFlags::NOEXEC;
    stage(
        Filesystem,
        os(mount(
            "tmpfs",
            &policy.root,
            "tmpfs",
            flags,
            Some(c"size=4096,nr_inodes=16,mode=0700"),
        )),
    )?;
    let scratch = policy.root.join("scratch");
    let old = policy.root.join("old-root");
    stage(Filesystem, fs::create_dir(&scratch))?;
    stage(Filesystem, fs::create_dir(&old))?;
    let options = std::ffi::CString::new(format!(
        "size={},nr_inodes={},mode=0700",
        policy.scratch_bytes, policy.scratch_inodes
    ))
    .map_err(|_| IsolationFailure {
        phase: Filesystem,
        kind: io::ErrorKind::InvalidInput,
        os_error: None,
    })?;
    stage(
        Filesystem,
        os(mount(
            "tmpfs",
            &scratch,
            "tmpfs",
            flags,
            Some(options.as_c_str()),
        )),
    )?;
    stage(Root, os(rustix::process::pivot_root(&policy.root, &old)))?;
    stage(Root, std::env::set_current_dir("/"))?;
    stage(Root, os(unmount("/old-root", UnmountFlags::DETACH)))?;
    stage(Root, fs::remove_dir("/old-root"))?;
    stage(Root, os(mount_remount("/", flags | MountFlags::RDONLY, "")))?;
    let scratch_stat = stage(Filesystem, os(rustix::fs::statvfs("/scratch")))?;
    let actual_bytes = scratch_stat
        .f_blocks
        .checked_mul(scratch_stat.f_frsize)
        .ok_or(IsolationFailure {
            phase: Filesystem,
            kind: io::ErrorKind::InvalidData,
            os_error: None,
        })?;
    if actual_bytes == 0
        || scratch_stat.f_files == 0
        || actual_bytes > policy.scratch_bytes
        || scratch_stat.f_files > policy.scratch_inodes
    {
        return Err(IsolationFailure {
            phase: Filesystem,
            kind: io::ErrorKind::PermissionDenied,
            os_error: None,
        });
    }
    stage(Privileges, os(set_no_new_privs(true)))?;
    stage(Privileges, os(clear_ambient_capability_set()))?;
    // Clear every known and future kernel bounding bit representable by this ABI.
    for bit in 0..64 {
        let cap = CapabilitySet::from_bits_retain(1u64 << bit);
        match capability_is_in_bounding_set(cap) {
            Ok(true) => stage(Privileges, os(remove_capability_from_bounding_set(cap)))?,
            Ok(false) => (),
            Err(rustix::io::Errno::INVAL) => break,
            Err(e) => {
                stage(Privileges, os::<()>(Err(e)))?;
            }
        }
    }
    stage(
        Privileges,
        os(set_capabilities_secure_bits(
            CapabilitiesSecureBits::NO_ROOT
                | CapabilitiesSecureBits::NO_ROOT_LOCKED
                | CapabilitiesSecureBits::NO_SETUID_FIXUP
                | CapabilitiesSecureBits::NO_SETUID_FIXUP_LOCKED,
        )),
    )?;
    let none = CapabilitySet::empty();
    stage(
        Privileges,
        os(set_capabilities(
            None,
            CapabilitySets {
                effective: none,
                permitted: none,
                inheritable: none,
            },
        )),
    )?;
    let actual = stage(Privileges, os(capabilities(None)))?;
    if !actual.effective.is_empty()
        || !actual.permitted.is_empty()
        || !actual.inheritable.is_empty()
        || !stage(Privileges, os(no_new_privs()))?
    {
        return Err(IsolationFailure {
            phase: Privileges,
            kind: io::ErrorKind::PermissionDenied,
            os_error: None,
        });
    }
    Ok(())
}

/// Purpose-built, finite qualification oracles used only by the explicit
/// isolated-probe-v1 fixture invocation, never by manager-selected execution.
/// Every target is a test-owned canary or loopback listener. No real secret or
/// external network target is accepted. The tmpfs ceiling is read back first.
pub(crate) fn probe(policy: &NativeIsolation, port: u16) -> io::Result<()> {
    use std::{
        io::Write,
        net::{Ipv4Addr, SocketAddrV4, TcpStream},
        os::unix::fs::symlink,
    };
    if policy.scratch_bytes != 65536 || policy.scratch_inodes != 16 || port == 0 {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    if !matches!(fs::write("/forbidden-write", b"owned-canary"), Err(e) if e.raw_os_error() == Some(30))
    {
        return Err(io::ErrorKind::PermissionDenied.into());
    }
    let outside = policy
        .root
        .parent()
        .ok_or(io::ErrorKind::InvalidInput)?
        .join("outside-canary");
    if fs::read(&outside).is_ok() {
        return Err(io::ErrorKind::PermissionDenied.into());
    }
    symlink(&outside, "/scratch/escape")?;
    if fs::read("/scratch/escape").is_ok() {
        return Err(io::ErrorKind::PermissionDenied.into());
    }
    fs::remove_file("/scratch/escape")?;
    // Absolute /../ remains rooted; there is no detached host-root descriptor.
    if fs::read("/../../proc/self/status").is_ok() {
        return Err(io::ErrorKind::PermissionDenied.into());
    }
    let scratch = rustix::fs::open(
        "/scratch",
        rustix::fs::OFlags::PATH | rustix::fs::OFlags::DIRECTORY,
        rustix::fs::Mode::empty(),
    )?;
    if rustix::fs::openat2(
        &scratch,
        "../outside-canary",
        rustix::fs::OFlags::RDONLY,
        rustix::fs::Mode::empty(),
        rustix::fs::ResolveFlags::BENEATH | rustix::fs::ResolveFlags::NO_SYMLINKS,
    )
    .is_ok()
    {
        return Err(io::ErrorKind::PermissionDenied.into());
    }
    let mut f = fs::File::create("/scratch/finite-quota")?;
    let block = [0x5a; 4096];
    for _ in 0..16 {
        f.write_all(&block)?;
    }
    let exceeded = f.write(&block);
    if !matches!(exceeded, Err(ref e) if e.raw_os_error() == Some(28)) {
        return Err(io::ErrorKind::InvalidData.into());
    }
    drop(f);
    fs::remove_file("/scratch/finite-quota")?;
    if TcpStream::connect_timeout(
        &SocketAddrV4::new(Ipv4Addr::LOCALHOST, port).into(),
        std::time::Duration::from_millis(50),
    )
    .is_ok()
    {
        return Err(io::ErrorKind::PermissionDenied.into());
    }
    Ok(())
}

/// Stable Linux process reference. Acquisition checks host boot/start identity
/// around pidfd_open; signalling never falls back to a recycled numeric PID.
/// Direct-process exit says nothing about descendants, cleanup or durable state.
pub(crate) struct StableProcess {
    fd: OwnedFd,
}
impl StableProcess {
    pub(crate) fn acquire(pid: u32, expected_ticks: u64) -> io::Result<Self> {
        let pid_value = i32::try_from(pid)
            .ok()
            .and_then(Pid::from_raw)
            .ok_or(io::ErrorKind::InvalidInput)?;
        if crate::domain_bootstrap::ticks(pid)? != expected_ticks {
            return Err(io::ErrorKind::InvalidData.into());
        }
        let fd = pidfd_open(pid_value, PidfdFlags::NONBLOCK)?;
        if crate::domain_bootstrap::ticks(pid)? != expected_ticks {
            return Err(io::ErrorKind::InvalidData.into());
        }
        let process = Self { fd };
        process.require_live()?;
        Ok(process)
    }
    pub(crate) fn require_live(&self) -> io::Result<()> {
        let mut fds = [PollFd::new(&self.fd, PollFlags::IN)];
        match poll(
            &mut fds,
            Some(&Timespec {
                tv_sec: 0,
                tv_nsec: 0,
            }),
        ) {
            Ok(0) => Ok(()),
            Ok(_) => Err(io::ErrorKind::NotFound.into()),
            Err(e) => Err(e.into()),
        }
    }
}

#[cfg(test)]
mod tests;
