//! Native structured systemd D-Bus transport for one fixed nonce service.
//! No external manager helper is spawned; the selected manager and cooperating
//! owner are trusted. Missing manager state is never evidence of noncreation.
use crate::{
    domain_bootstrap::{connect_now, ticks, token},
    supervisor::PinnedExecutable,
};
use cortex_core::{
    program::{OperationId, domain::*},
    temporal::SourceReference,
};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    fs,
    future::Future,
    io::{self, Read},
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};
const MANAGER_PATH: &str = "/org/freedesktop/systemd1";
const MANAGER_INTERFACE: &str = "org.freedesktop.systemd1.Manager";
/// Exact failure phase; no raw tool output, arguments or private paths are exposed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DomainPhaseDiagnostic {
    /// Read-only preparation.
    Preparation,
    /// Durable create followed by manager submission.
    Creation,
    /// Actual incarnation acquisition.
    Identity,
    /// Durable bind/release publication.
    Publication,
    /// One-shot bootstrap release.
    Release,
    /// Whole-domain stop observation.
    Closure,
}
/// Bounded redacted cause. Missing units are never converted to noncreation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DomainCause {
    /// Unsupported/malformed selected contract.
    Contract,
    /// Immutable correspondence changed.
    Correspondence,
    /// Manager failed without a structured successful reply.
    ManagerRefusal,
    /// Selected deadline elapsed.
    Deadline,
    /// Output/transport limit exceeded.
    Capacity,
    /// Response shape is unsupported.
    Protocol,
    /// Current authority or cancellation blocks effects.
    Authority,
    /// Durable publication was refused or uncertain.
    Journal,
    /// Safe local I/O category.
    Io(io::ErrorKind),
}
/// Safe diagnostic; this observation is not an outcome or resource refund.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainDiagnostic {
    /// Failed protocol phase.
    pub phase: DomainPhaseDiagnostic,
    /// Typed redacted cause.
    pub cause: DomainCause,
    /// Structured D-Bus error identity only; raw messages are never retained.
    pub manager_error: Option<String>,
}
impl std::fmt::Display for DomainDiagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "domain {:?}: {:?}", self.phase, self.cause)
    }
}
impl std::error::Error for DomainDiagnostic {}
pub(crate) fn issue(phase: DomainPhaseDiagnostic, cause: DomainCause) -> DomainDiagnostic {
    DomainDiagnostic {
        phase,
        cause,
        manager_error: None,
    }
}

/// Explicit service controls. These are configured values, not qualification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceLimits {
    /// Bootstrap lifetime in milliseconds, at most the fixed profile's 5000.
    pub wall_ms: u64,
    /// CPU quota microseconds per period.
    pub cpu_quota_us: u64,
    /// CPU period microseconds.
    pub cpu_period_us: u64,
    /// Charged memory ceiling.
    pub memory_bytes: u64,
    /// Swap ceiling (zero for the initial profile).
    pub swap_bytes: u64,
    /// Cgroup tasks/threads ceiling.
    pub tasks: u64,
}
/// Explicit native inputs. No field accepts a payload executable or provider.
#[derive(Clone)]
pub struct ServiceSelection {
    /// Exact trusted OS environment cleaner, invoked with fixed -i arguments.
    pub environment_cleaner: PathBuf,
    /// Exact compiled fixed Cortex bootstrap.
    pub bootstrap: PathBuf,
    /// Existing owner-only controller directory; it must not be a home tree copy.
    pub controller_dir: PathBuf,
    /// Exact owner runtime directory containing the session bus.
    pub runtime_dir: PathBuf,
    /// Existing legitimately delegated cgroup root.
    pub delegated_root: PathBuf,
    /// Explicit finite executable read/pinning ceiling.
    pub executable_bytes: u64,
    /// Exact public synthetic nonce bound to the immutable configuration.
    pub nonce: String,
    /// Explicit service controls.
    pub limits: ServiceLimits,
}

/// Read-only prepared direct D-Bus adapter. It does not activate a backend and
/// authenticates only the existing local bus connection, not owner approval.
pub struct SystemdNonceDomain {
    pub(crate) selection: ServiceSelection,
    cleaner: PinnedExecutable,
    bootstrap: PinnedExecutable,
    connection: zbus::Connection,
    manager: String,
    boot: String,
    configuration: SourceReference,
}
#[derive(Clone)]
pub(crate) struct ManagedObservation {
    pub identity: DomainIdentity,
    pub main_pid: u32,
    pub start_ticks: u64,
    pub populated: bool,
    pub direct_members: Vec<DomainProcessObservation>,
    pub controls: DomainControlReadback,
    pub cpu_micros: Option<u64>,
    pub memory_peak: Option<u64>,
    pub restart_prevented: bool,
}
impl SystemdNonceDomain {
    /// Inspect exact inputs and connect to the existing user bus with a bounded
    /// authentication deadline. No autolaunch address, shell, unit or helper is
    /// used. A controlled trusted manager is assumed; this is not qualification.
    pub fn inspect(selection: ServiceSelection) -> Result<Self, DomainDiagnostic> {
        let phase = DomainPhaseDiagnostic::Preparation;
        let end = Instant::now() + Duration::from_secs(1);
        let l = &selection.limits;
        if !token(&selection.nonce)
            || l.wall_ms == 0
            || l.wall_ms > 5000
            || l.cpu_quota_us == 0
            || l.cpu_period_us == 0
            || l.memory_bytes == 0
            || l.swap_bytes != 0
            || l.tasks == 0
            || l.tasks > 8
        {
            return Err(issue(phase, DomainCause::Contract));
        }
        for path in [
            &selection.controller_dir,
            &selection.runtime_dir,
            &selection.delegated_root,
        ] {
            let m =
                fs::symlink_metadata(path).map_err(|e| issue(phase, DomainCause::Io(e.kind())))?;
            if !m.is_dir()
                || fs::canonicalize(path).ok().as_ref() != Some(path)
                || m.mode() & 0o022 != 0
            {
                return Err(issue(phase, DomainCause::Contract));
            }
        }
        let directory = fs::metadata(&selection.controller_dir)
            .map_err(|e| issue(phase, DomainCause::Io(e.kind())))?;
        if directory.mode() & 0o077 != 0 {
            return Err(issue(phase, DomainCause::Contract));
        }
        let cleaner =
            PinnedExecutable::inspect(&selection.environment_cleaner, selection.executable_bytes)
                .map_err(|_| issue(phase, DomainCause::Correspondence))?;
        let bootstrap = PinnedExecutable::inspect(&selection.bootstrap, selection.executable_bytes)
            .map_err(|_| issue(phase, DomainCause::Correspondence))?;
        let boot = fs::read_to_string("/proc/sys/kernel/random/boot_id")
            .map_err(|e| issue(phase, DomainCause::Io(e.kind())))?
            .trim()
            .to_string();

        let stream = connect_now(&selection.runtime_dir.join("bus"))
            .map_err(|e| issue(phase, DomainCause::Io(e.kind())))?;
        // One absolute deadline includes connect, authentication and Hello.
        let connection = bounded(
            zbus::connection::Builder::async_io_unix_stream(stream)
                .max_queued(8)
                .method_timeout(Duration::from_secs(1))
                .build(),
            phase,
            end,
        )?;
        let reply = bounded(
            connection.call_method(
                Some("org.freedesktop.DBus"),
                "/org/freedesktop/DBus",
                Some("org.freedesktop.DBus"),
                "GetNameOwner",
                &("org.freedesktop.systemd1",),
            ),
            phase,
            end,
        )?;
        let manager: String = reply
            .body()
            .deserialize()
            .map_err(|_| issue(phase, DomainCause::Protocol))?;
        if !manager.starts_with(':') || manager.len() > 128 {
            return Err(issue(phase, DomainCause::Protocol));
        }
        let configuration = SourceReference::operator_reported(
            "selected-native-backend",
            "systemd-nonce-v1",
            &configuration_bytes(&selection, &manager, connection.server_guid().as_str())?,
        )
        .map_err(|_| issue(phase, DomainCause::Capacity))?;
        Ok(Self {
            selection,
            cleaner,
            bootstrap,
            connection,
            manager,
            boot,
            configuration,
        })
    }
    /// Exact selected configuration and SHA-256 executable correspondence.
    pub fn configuration(&self) -> &SourceReference {
        &self.configuration
    }
    /// The profile intentionally has no arbitrary executable/provider transport.
    pub fn support_profile(&self) -> &'static str {
        "fixed-public-nonce-only; host-unqualified; no-arbitrary-exec; no-provider; scratch-and-hard-cumulative-cpu-unsupported"
    }
    pub(crate) fn endpoint(&self, request: &OperationId) -> Result<PathBuf, DomainDiagnostic> {
        endpoint(&self.selection, request)
    }
    pub(crate) fn corresponds(&self, intent: &DomainIntent) -> Result<(), DomainDiagnostic> {
        let phase = DomainPhaseDiagnostic::Identity;
        if intent.configuration != self.configuration || intent.pool.boot != self.boot {
            return Err(issue(phase, DomainCause::Correspondence));
        }
        use cortex_core::program::scheduling::Resource;
        if intent.effect.demand.iter().any(|d| match &d.resource {
            Resource::UnspecifiedMemoryBytes => true,
            Resource::MemoryBytes(m) => {
                m.basis() != cortex_core::program::memory::MemoryBasis::CgroupCharged
                    || !m.accounts_pool(&intent.pool)
            }
            _ => false,
        }) {
            return Err(issue(phase, DomainCause::Correspondence));
        }
        let l = &self.selection.limits;
        let required_cpu = l
            .cpu_quota_us
            .checked_mul(1000)
            .and_then(|n| n.checked_add(l.cpu_period_us - 1))
            .and_then(|n| n.checked_div(l.cpu_period_us))
            .ok_or_else(|| issue(phase, DomainCause::Contract))?;
        for (resource, amount) in [
            (Resource::WorkerSlots, 1),
            (
                Resource::memory(
                    cortex_core::program::memory::MemoryBasis::CgroupCharged,
                    cortex_core::program::memory::MemoryScope::AccountingPool(intent.pool.clone()),
                )
                .map_err(|_| issue(phase, DomainCause::Contract))?,
                l.memory_bytes,
            ),
            (Resource::CpuMillicores, required_cpu),
        ] {
            if intent
                .effect
                .demand
                .iter()
                .find(|d| d.resource.clone() == resource)
                .and_then(|d| d.amount)
                .is_none_or(|n| n < amount)
            {
                return Err(issue(phase, DomainCause::Correspondence));
            }
        }
        let release_bytes = serde_json::to_vec(&json!([
            1,
            "release",
            intent.request.as_str(),
            self.selection.nonce
        ]))
        .map_err(|_| issue(phase, DomainCause::Protocol))?
        .len() as u64
            + 4;
        for (dimension, amount) in [
            (Consumption::ElapsedMillis, l.wall_ms),
            (Consumption::InputBytes, release_bytes),
            (
                Consumption::OutputBytes,
                2 * (crate::domain_bootstrap::FRAME_LIMIT as u64 + 4),
            ),
            (Consumption::Events, 4),
        ] {
            if intent
                .consumption
                .iter()
                .find(|d| d.dimension == dimension)
                .is_none_or(|d| d.amount < amount)
            {
                return Err(issue(phase, DomainCause::Correspondence));
            }
        }
        if !intent
            .consumption
            .iter()
            .any(|d| d.dimension == Consumption::CpuExecutionMillis && d.amount > 0)
        {
            return Err(issue(phase, DomainCause::Correspondence));
        }
        self.cleaner
            .check()
            .and_then(|_| self.bootstrap.check())
            .map_err(|_| issue(phase, DomainCause::Correspondence))?;
        let boot = fs::read_to_string("/proc/sys/kernel/random/boot_id")
            .map_err(|e| issue(phase, DomainCause::Io(e.kind())))?;
        if boot.trim() != self.boot
            || configuration_bytes(
                &self.selection,
                &self.manager,
                self.connection.server_guid().as_str(),
            )? != self.configuration.version()
        {
            return Err(issue(phase, DomainCause::Correspondence));
        }
        Ok(())
    }
    pub(crate) fn create(
        &mut self,
        intent: &DomainIntent,
        end: Instant,
    ) -> Result<(), DomainDiagnostic> {
        self.corresponds(intent)?;
        let properties = creation_properties(&self.selection, intent)?;
        let auxiliaries: Vec<(String, Vec<(String, OwnedValue)>)> = vec![];
        let body = (unit(intent), "fail", properties, auxiliaries);
        let reply = bounded(
            self.connection.call_method(
                Some(self.manager.as_str()),
                MANAGER_PATH,
                Some(MANAGER_INTERFACE),
                "StartTransientUnit",
                &body,
            ),
            DomainPhaseDiagnostic::Creation,
            end,
        )?;
        let _: OwnedObjectPath = reply
            .body()
            .deserialize()
            .map_err(|_| issue(DomainPhaseDiagnostic::Creation, DomainCause::Protocol))?;
        Ok(())
    }
    fn property(
        &self,
        object: &OwnedObjectPath,
        interface: &str,
        name: &str,
        end: Instant,
    ) -> Result<OwnedValue, DomainDiagnostic> {
        let reply = bounded(
            self.connection.call_method(
                Some(self.manager.as_str()),
                object.as_str(),
                Some("org.freedesktop.DBus.Properties"),
                "Get",
                &(interface, name),
            ),
            DomainPhaseDiagnostic::Identity,
            end,
        )?;
        if reply.data().len() > 65_536 {
            return Err(issue(
                DomainPhaseDiagnostic::Identity,
                DomainCause::Capacity,
            ));
        }
        reply
            .body()
            .deserialize()
            .map_err(|_| issue(DomainPhaseDiagnostic::Identity, DomainCause::Protocol))
    }
    pub(crate) fn observe(
        &mut self,
        intent: &DomainIntent,
        end: Instant,
    ) -> Result<ManagedObservation, DomainDiagnostic> {
        self.read_observation(intent, end, false)
    }
    fn read_observation(
        &mut self,
        intent: &DomainIntent,
        end: Instant,
        cleanup: bool,
    ) -> Result<ManagedObservation, DomainDiagnostic> {
        self.corresponds(intent)?;
        let reply = bounded(
            self.connection.call_method(
                Some(self.manager.as_str()),
                MANAGER_PATH,
                Some(MANAGER_INTERFACE),
                "GetUnit",
                &(unit(intent),),
            ),
            DomainPhaseDiagnostic::Identity,
            end,
        )?;
        let object: OwnedObjectPath = reply
            .body()
            .deserialize()
            .map_err(|_| issue(DomainPhaseDiagnostic::Identity, DomainCause::Protocol))?;
        let ui = "org.freedesktop.systemd1.Unit";
        let si = "org.freedesktop.systemd1.Service";
        let description_actual = as_string(self.property(&object, ui, "Description", end)?)?;
        let restart = as_string(self.property(&object, si, "Restart", end)?)?;
        let mode = as_string(self.property(&object, si, "KillMode", end)?)?;
        if description_actual != description(intent) || restart != "no" || mode != "control-group" {
            return Err(issue(
                DomainPhaseDiagnostic::Identity,
                DomainCause::Correspondence,
            ));
        }
        let exec: Vec<ServiceCommandObservation> = self
            .property(&object, si, "ExecStart", end)?
            .try_into()
            .map_err(|_| issue(DomainPhaseDiagnostic::Identity, DomainCause::Protocol))?;
        let endpoint = self.endpoint(&intent.request)?;
        let cleaner = self
            .selection
            .environment_cleaner
            .to_str()
            .ok_or_else(|| issue(DomainPhaseDiagnostic::Identity, DomainCause::Contract))?;
        let bootstrap = self
            .selection
            .bootstrap
            .to_str()
            .ok_or_else(|| issue(DomainPhaseDiagnostic::Identity, DomainCause::Contract))?;
        let expected = vec![
            cleaner.to_string(),
            "-i".into(),
            bootstrap.to_string(),
            endpoint
                .to_str()
                .ok_or_else(|| issue(DomainPhaseDiagnostic::Identity, DomainCause::Contract))?
                .into(),
            intent.request.as_str().into(),
            self.selection.limits.wall_ms.to_string(),
        ];
        if exec.len() != 1 || exec[0].0 != cleaner || exec[0].1 != expected || exec[0].2 {
            return Err(issue(
                DomainPhaseDiagnostic::Identity,
                DomainCause::Correspondence,
            ));
        }
        let nnp: bool = self
            .property(&object, si, "NoNewPrivileges", end)?
            .try_into()
            .map_err(|_| issue(DomainPhaseDiagnostic::Identity, DomainCause::Protocol))?;
        if !nnp {
            return Err(issue(
                DomainPhaseDiagnostic::Identity,
                DomainCause::Correspondence,
            ));
        }
        let runtime_max: u64 = self
            .property(&object, si, "RuntimeMaxUSec", end)?
            .try_into()
            .map_err(|_| issue(DomainPhaseDiagnostic::Identity, DomainCause::Protocol))?;
        let timeout_stop: u64 = self
            .property(&object, si, "TimeoutStopUSec", end)?
            .try_into()
            .map_err(|_| issue(DomainPhaseDiagnostic::Identity, DomainCause::Protocol))?;
        let send_sigkill: bool = self
            .property(&object, si, "SendSIGKILL", end)?
            .try_into()
            .map_err(|_| issue(DomainPhaseDiagnostic::Identity, DomainCause::Protocol))?;
        let remain_after_exit: bool = self
            .property(&object, si, "RemainAfterExit", end)?
            .try_into()
            .map_err(|_| issue(DomainPhaseDiagnostic::Identity, DomainCause::Protocol))?;
        if runtime_max != self.selection.limits.wall_ms * 1000
            || timeout_stop != 500_000
            || !send_sigkill
            || !remain_after_exit
        {
            return Err(issue(
                DomainPhaseDiagnostic::Identity,
                DomainCause::Correspondence,
            ));
        }
        let invocation: Vec<u8> = self
            .property(&object, ui, "InvocationID", end)?
            .try_into()
            .map_err(|_| issue(DomainPhaseDiagnostic::Identity, DomainCause::Protocol))?;
        if invocation.len() != 16 || invocation.iter().all(|b| *b == 0) {
            return Err(issue(
                DomainPhaseDiagnostic::Identity,
                DomainCause::Correspondence,
            ));
        }
        let invocation = invocation
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        let cgroup = as_string(self.property(&object, ui, "ControlGroup", end)?)?;
        if !cgroup.starts_with('/') || cgroup.contains("..") || cgroup.len() > 1024 {
            return Err(issue(
                DomainPhaseDiagnostic::Identity,
                DomainCause::Correspondence,
            ));
        }
        let path = Path::new("/sys/fs/cgroup").join(cgroup.trim_start_matches('/'));
        if !path.starts_with(&self.selection.delegated_root)
            || path == self.selection.delegated_root
            || fs::canonicalize(&path).ok().as_ref() != Some(&path)
        {
            return Err(issue(
                DomainPhaseDiagnostic::Identity,
                DomainCause::Correspondence,
            ));
        }
        let m = fs::metadata(&path)
            .map_err(|e| issue(DomainPhaseDiagnostic::Identity, DomainCause::Io(e.kind())))?;
        let populated =
            match key_number(&bounded_metadata(&path.join("cgroup.events"))?, "populated") {
                Some(0) => false,
                Some(1) => true,
                _ => {
                    return Err(issue(
                        DomainPhaseDiagnostic::Identity,
                        DomainCause::Protocol,
                    ));
                }
            };
        let main: u32 = self
            .property(&object, si, "MainPID", end)?
            .try_into()
            .map_err(|_| issue(DomainPhaseDiagnostic::Identity, DomainCause::Protocol))?;
        let start = if main == 0 {
            0
        } else {
            ticks(main)
                .map_err(|e| issue(DomainPhaseDiagnostic::Identity, DomainCause::Io(e.kind())))?
        };
        let require_main_membership = if cleanup { main != 0 } else { populated };
        if require_main_membership
            && (main == 0
                || bounded_metadata(&PathBuf::from(format!("/proc/{main}/cgroup")))?.trim()
                    != format!("0::{cgroup}"))
        {
            return Err(issue(
                DomainPhaseDiagnostic::Identity,
                DomainCause::Correspondence,
            ));
        }
        // Retain direct members only. This is not a whole-subtree or atomic
        // process-set proof; a changed list or birth identity refuses binding.
        let observed_members = member_ids(&bounded_metadata(&path.join("cgroup.procs"))?)?;
        let mut direct_members = Vec::with_capacity(observed_members.len());
        for pid in &observed_members {
            if Instant::now() >= end {
                return Err(issue(
                    DomainPhaseDiagnostic::Identity,
                    DomainCause::Deadline,
                ));
            }
            let start_ticks = ticks(*pid)
                .map_err(|e| issue(DomainPhaseDiagnostic::Identity, DomainCause::Io(e.kind())))?;
            if bounded_metadata(&PathBuf::from(format!("/proc/{pid}/cgroup")))?.trim()
                != format!("0::{cgroup}")
                || ticks(*pid).ok() != Some(start_ticks)
            {
                return Err(issue(
                    DomainPhaseDiagnostic::Identity,
                    DomainCause::Correspondence,
                ));
            }
            direct_members.push(DomainProcessObservation {
                pid: *pid,
                start_ticks,
            });
        }
        if member_ids(&bounded_metadata(&path.join("cgroup.procs"))?)? != observed_members
            || (require_main_membership
                && !direct_members.contains(&DomainProcessObservation {
                    pid: main,
                    start_ticks: start,
                }))
        {
            return Err(issue(
                DomainPhaseDiagnostic::Identity,
                DomainCause::Correspondence,
            ));
        }
        let l = &self.selection.limits;
        if bounded_metadata(&path.join("cpu.max"))?.trim()
            != format!("{} {}", l.cpu_quota_us, l.cpu_period_us)
            || bounded_metadata(&path.join("memory.max"))?.trim() != l.memory_bytes.to_string()
            || bounded_metadata(&path.join("memory.swap.max"))?.trim() != l.swap_bytes.to_string()
            || bounded_metadata(&path.join("pids.max"))?.trim() != l.tasks.to_string()
        {
            return Err(issue(
                DomainPhaseDiagnostic::Identity,
                DomainCause::Correspondence,
            ));
        }
        let cpu = bounded_metadata(&path.join("cpu.stat"))
            .ok()
            .and_then(|v| key_number(&v, "usage_usec"));
        let memory = bounded_metadata(&path.join("memory.peak"))
            .ok()
            .and_then(|v| v.trim().parse().ok());
        if Instant::now() >= end {
            return Err(issue(
                DomainPhaseDiagnostic::Identity,
                DomainCause::Deadline,
            ));
        }
        Ok(ManagedObservation {
            identity: DomainIdentity {
                pool: intent.pool.clone(),
                request: intent.request.clone(),
                manager: format!("{}|{}", self.connection.server_guid(), self.manager),
                invocation,
                control_group: cgroup,
                cgroup_device: m.dev(),
                cgroup_inode: m.ino(),
                boot: self.boot.clone(),
            },
            main_pid: main,
            start_ticks: start,
            populated,
            direct_members,
            controls: DomainControlReadback {
                cpu_quota_us: l.cpu_quota_us,
                cpu_period_us: l.cpu_period_us,
                memory_bytes: l.memory_bytes,
                swap_bytes: l.swap_bytes,
                tasks: l.tasks,
                runtime_max_micros: runtime_max,
                timeout_stop_micros: timeout_stop,
                restart_prevented: true,
                kill_control_group: true,
                no_new_privileges: nnp,
                send_sigkill,
                remain_after_exit,
            },
            cpu_micros: cpu,
            memory_peak: memory,
            restart_prevented: true,
        })
    }
    pub(crate) fn observe_tree(
        &mut self,
        intent: &DomainIntent,
        end: Instant,
    ) -> Result<(ManagedObservation, Vec<DomainCgroupObservation>), DomainDiagnostic> {
        let before = self.read_observation(intent, end, true)?;
        let path =
            Path::new("/sys/fs/cgroup").join(before.identity.control_group.trim_start_matches('/'));
        let nodes = crate::domain_tree::capture(
            &path,
            before.identity.cgroup_device,
            before.identity.cgroup_inode,
            &before.identity.control_group,
            end,
            |pid, group| {
                let (start_ticks, before_bytes) = bounded_ticks(pid)?;
                let membership = bounded_metadata(&PathBuf::from(format!("/proc/{pid}/cgroup")))?;
                let (after_ticks, after_bytes) = bounded_ticks(pid)?;
                if membership.trim() != format!("0::{group}") || after_ticks != start_ticks {
                    return Err(issue(
                        DomainPhaseDiagnostic::Closure,
                        DomainCause::Correspondence,
                    ));
                }
                Ok((
                    DomainProcessObservation { pid, start_ticks },
                    before_bytes + membership.len() + after_bytes,
                ))
            },
        )?;
        let after = self.read_observation(intent, end, true)?;
        let root = nodes
            .first()
            .ok_or_else(|| issue(DomainPhaseDiagnostic::Closure, DomainCause::Correspondence))?;
        if before.identity != after.identity
            || root.populated != after.populated
            || root.members != after.direct_members
        {
            return Err(issue(
                DomainPhaseDiagnostic::Closure,
                DomainCause::Correspondence,
            ));
        }
        Ok((after, nodes))
    }
    pub(crate) fn stop(
        &mut self,
        intent: &DomainIntent,
        identity: &DomainIdentity,
        end: Instant,
    ) -> Result<(), DomainDiagnostic> {
        // Cleanup can target the exact still-retained incarnation even after
        // its main exits. This reads the same manager/configuration/cgroup
        // correspondence; preparation and release keep the strict live-main path.
        let observed = self.read_observation(intent, end, true)?;
        checked_stop_request(&observed, identity, || {
            let reply = bounded(
                self.connection.call_method(
                    Some(self.manager.as_str()),
                    MANAGER_PATH,
                    Some(MANAGER_INTERFACE),
                    "StopUnit",
                    &(unit(intent), "fail"),
                ),
                DomainPhaseDiagnostic::Closure,
                end,
            )?;
            let _: OwnedObjectPath = reply
                .body()
                .deserialize()
                .map_err(|_| issue(DomainPhaseDiagnostic::Closure, DomainCause::Protocol))?;
            Ok(())
        })
    }
}
/// Gate a single stop request, not a claim of stop success or quiescence. The
/// manager's name-based call still assumes the documented cooperating operator
/// and trusted manager; it is not an atomic conditional StopUnit operation.
fn checked_stop_request<T>(
    observed: &ManagedObservation,
    expected: &DomainIdentity,
    request: impl FnOnce() -> Result<T, DomainDiagnostic>,
) -> Result<T, DomainDiagnostic> {
    observed
        .identity
        .validate()
        .map_err(|_| issue(DomainPhaseDiagnostic::Closure, DomainCause::Correspondence))?;
    expected
        .validate()
        .map_err(|_| issue(DomainPhaseDiagnostic::Closure, DomainCause::Correspondence))?;
    if observed.identity != *expected {
        return Err(issue(
            DomainPhaseDiagnostic::Closure,
            DomainCause::Correspondence,
        ));
    }
    request()
}
fn bounded_ticks(pid: u32) -> Result<(u64, usize), DomainDiagnostic> {
    let text = bounded_metadata(&PathBuf::from(format!("/proc/{pid}/stat")))?;
    let tail = text
        .rsplit_once(')')
        .ok_or_else(|| issue(DomainPhaseDiagnostic::Closure, DomainCause::Protocol))?
        .1;
    let ticks = tail
        .split_whitespace()
        .nth(19)
        .and_then(|value| value.parse().ok())
        .ok_or_else(|| issue(DomainPhaseDiagnostic::Closure, DomainCause::Protocol))?;
    Ok((ticks, text.len()))
}
fn member_ids(value: &str) -> Result<Vec<u32>, DomainDiagnostic> {
    let mut members = std::collections::BTreeSet::new();
    for part in value.split_whitespace() {
        let pid: u32 = part
            .parse()
            .map_err(|_| issue(DomainPhaseDiagnostic::Identity, DomainCause::Protocol))?;
        if pid == 0 || !members.insert(pid) || members.len() > 8 {
            return Err(issue(
                DomainPhaseDiagnostic::Identity,
                DomainCause::Capacity,
            ));
        }
    }
    Ok(members.into_iter().collect())
}
fn bounded<T>(
    future: impl Future<Output = zbus::Result<T>>,
    phase: DomainPhaseDiagnostic,
    end: Instant,
) -> Result<T, DomainDiagnostic> {
    if Instant::now() >= end {
        return Err(issue(phase, DomainCause::Deadline));
    }
    let value = futures_lite::future::block_on(futures_lite::future::or(
        async {
            future.await.map_err(|e| {
                let cause = match e {
                    zbus::Error::InputOutput(ref e) if e.kind() == io::ErrorKind::TimedOut => {
                        DomainCause::Deadline
                    }
                    _ => DomainCause::ManagerRefusal,
                };
                {
                    let mut diagnostic = issue(phase, cause);
                    if let zbus::Error::MethodError(name, _, _) = e {
                        let name = name.as_str();
                        if name.len() <= 256 {
                            diagnostic.manager_error = Some(name.into())
                        }
                    }
                    diagnostic
                }
            })
        },
        async {
            async_io::Timer::at(end).await;
            Err(issue(phase, DomainCause::Deadline))
        },
    ))?;
    if Instant::now() >= end {
        return Err(issue(phase, DomainCause::Deadline));
    }
    Ok(value)
}
fn unit(intent: &DomainIntent) -> String {
    format!("cortex-domain-{}.service", intent.request.as_str())
}
fn description(intent: &DomainIntent) -> String {
    format!(
        "Cortex nonce v1 {} {}",
        intent.request.as_str(),
        format_args!(
            "{:x}",
            Sha256::digest(intent.configuration.version().as_bytes())
        )
    )
}
// systemd's read-only ExecStart observation extends the creation tuple with
// timestamps, process ID and exit observations. These are not acceptance.
type ServiceCommandObservation = (String, Vec<String>, bool, u64, u64, u64, u64, u32, i32, i32);
fn configuration_bytes(
    s: &ServiceSelection,
    manager: &str,
    bus: &str,
) -> Result<String, DomainDiagnostic> {
    let mut paths = vec![];
    for p in [
        &s.environment_cleaner,
        &s.bootstrap,
        &s.controller_dir,
        &s.runtime_dir,
        &s.delegated_root,
    ] {
        let m = fs::metadata(p).map_err(|e| {
            issue(
                DomainPhaseDiagnostic::Preparation,
                DomainCause::Io(e.kind()),
            )
        })?;
        let digest = if m.is_file() {
            let mut bytes = vec![];
            fs::File::open(p)
                .and_then(|f| {
                    f.take(s.executable_bytes.saturating_add(1))
                        .read_to_end(&mut bytes)
                })
                .map_err(|e| {
                    issue(
                        DomainPhaseDiagnostic::Preparation,
                        DomainCause::Io(e.kind()),
                    )
                })?;
            if bytes.len() as u64 > s.executable_bytes {
                return Err(issue(
                    DomainPhaseDiagnostic::Preparation,
                    DomainCause::Capacity,
                ));
            }
            format!("{:x}", Sha256::digest(&bytes))
        } else {
            String::new()
        };
        paths.push(json!([
            p.to_str(),
            m.dev(),
            m.ino(),
            m.mode(),
            m.uid(),
            digest
        ]));
    }
    let l = &s.limits;
    let value = json!([
        1,
        "native-dbus-fixed-nonce",
        manager,
        bus,
        paths,
        s.nonce,
        l.wall_ms,
        l.cpu_quota_us,
        l.cpu_period_us,
        l.memory_bytes,
        l.swap_bytes,
        l.tasks
    ])
    .to_string();
    if value.len() > 3000 {
        return Err(issue(
            DomainPhaseDiagnostic::Preparation,
            DomainCause::Capacity,
        ));
    }
    Ok(value)
}
fn as_string(v: OwnedValue) -> Result<String, DomainDiagnostic> {
    String::try_from(v).map_err(|_| issue(DomainPhaseDiagnostic::Identity, DomainCause::Protocol))
}
fn bounded_metadata(path: &Path) -> Result<String, DomainDiagnostic> {
    let mut bytes = vec![];
    fs::File::open(path)
        .and_then(|f| f.take(16_385).read_to_end(&mut bytes))
        .map_err(|e| issue(DomainPhaseDiagnostic::Identity, DomainCause::Io(e.kind())))?;
    if bytes.len() > 16_384 {
        return Err(issue(
            DomainPhaseDiagnostic::Identity,
            DomainCause::Capacity,
        ));
    }
    String::from_utf8(bytes)
        .map_err(|_| issue(DomainPhaseDiagnostic::Identity, DomainCause::Protocol))
}
fn key_number(text: &str, key: &str) -> Option<u64> {
    let mut values = text.lines().filter_map(|l| {
        let (k, v) = l.split_once(' ')?;
        (k == key).then(|| v.parse::<u64>().ok()).flatten()
    });
    let first = values.next()?;
    values.next().is_none().then_some(first)
}

fn creation_properties(
    selection: &ServiceSelection,
    intent: &DomainIntent,
) -> Result<Vec<(String, OwnedValue)>, DomainDiagnostic> {
    let phase = DomainPhaseDiagnostic::Preparation;
    let l = &selection.limits;
    let quota = l
        .cpu_quota_us
        .checked_mul(1_000_000)
        .and_then(|n| n.checked_div(l.cpu_period_us))
        .ok_or_else(|| issue(phase, DomainCause::Contract))?;
    let endpoint = endpoint(selection, &intent.request)?;
    let cleaner = selection
        .environment_cleaner
        .to_str()
        .ok_or_else(|| issue(phase, DomainCause::Contract))?;
    let bootstrap = selection
        .bootstrap
        .to_str()
        .ok_or_else(|| issue(phase, DomainCause::Contract))?;
    let endpoint = endpoint
        .to_str()
        .ok_or_else(|| issue(phase, DomainCause::Contract))?;
    let exec = vec![(
        cleaner.to_string(),
        vec![
            cleaner.to_string(),
            "-i".into(),
            bootstrap.to_string(),
            endpoint.to_string(),
            intent.request.as_str().into(),
            l.wall_ms.to_string(),
        ],
        false,
    )];
    let text = |s: String| {
        Value::from(s)
            .try_to_owned()
            .map_err(|_| issue(phase, DomainCause::Protocol))
    };
    Ok(vec![
        ("Description".into(), text(description(intent))?),
        ("Type".into(), text("exec".into())?),
        ("Restart".into(), text("no".into())?),
        ("RemainAfterExit".into(), true.into()),
        ("KillMode".into(), text("control-group".into())?),
        ("SendSIGKILL".into(), true.into()),
        ("NoNewPrivileges".into(), true.into()),
        ("StandardInput".into(), text("null".into())?),
        ("StandardOutput".into(), text("null".into())?),
        ("StandardError".into(), text("null".into())?),
        ("RuntimeMaxUSec".into(), (l.wall_ms * 1000).into()),
        ("TimeoutStopUSec".into(), 500_000u64.into()),
        ("CPUQuotaPerSecUSec".into(), quota.into()),
        ("CPUQuotaPeriodUSec".into(), l.cpu_period_us.into()),
        ("MemoryMax".into(), l.memory_bytes.into()),
        ("MemorySwapMax".into(), l.swap_bytes.into()),
        ("TasksMax".into(), l.tasks.into()),
        (
            "ExecStart".into(),
            Value::new(exec)
                .try_to_owned()
                .map_err(|_| issue(phase, DomainCause::Protocol))?,
        ),
    ])
}

fn endpoint(
    selection: &ServiceSelection,
    request: &OperationId,
) -> Result<PathBuf, DomainDiagnostic> {
    let value = request.as_str();
    if !token(value) || value.len() > 40 {
        return Err(issue(
            DomainPhaseDiagnostic::Preparation,
            DomainCause::Contract,
        ));
    }
    let p = selection.controller_dir.join(format!("{value}.sock"));
    if p.as_os_str().len() > 100 {
        return Err(issue(
            DomainPhaseDiagnostic::Preparation,
            DomainCause::Contract,
        ));
    }
    Ok(p)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    #[test]
    fn persisted_configuration_binds_bytes_bus_nonce_and_all_limits() {
        static N: AtomicU64 = AtomicU64::new(0);
        let p = std::env::temp_dir().join(format!(
            "cortex-domain-config-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&p).unwrap();
        let binary = p.join("owned-binary");
        fs::write(&binary, b"original").unwrap();
        let mut selection = ServiceSelection {
            environment_cleaner: binary.clone(),
            bootstrap: binary.clone(),
            controller_dir: p.clone(),
            runtime_dir: p.clone(),
            delegated_root: p.clone(),
            executable_bytes: 1024,
            nonce: "public".into(),
            limits: ServiceLimits {
                wall_ms: 100,
                cpu_quota_us: 100000,
                cpu_period_us: 100000,
                memory_bytes: 1024,
                swap_bytes: 0,
                tasks: 2,
            },
        };
        let original = configuration_bytes(&selection, ":1.1", "bus-1").unwrap();
        // Same path, inode and byte length; content correspondence must change.
        fs::write(&binary, b"replaced").unwrap();
        assert_ne!(
            configuration_bytes(&selection, ":1.1", "bus-1").unwrap(),
            original
        );
        fs::write(&binary, b"original").unwrap();
        assert_eq!(
            configuration_bytes(&selection, ":1.1", "bus-1").unwrap(),
            original
        );
        assert_ne!(
            configuration_bytes(&selection, ":1.1", "bus-2").unwrap(),
            original
        );
        assert_ne!(
            configuration_bytes(&selection, ":1.2", "bus-1").unwrap(),
            original
        );
        selection.nonce = "different".into();
        assert_ne!(
            configuration_bytes(&selection, ":1.1", "bus-1").unwrap(),
            original
        );
        selection.nonce = "public".into();
        selection.limits.tasks = 3;
        assert_ne!(
            configuration_bytes(&selection, ":1.1", "bus-1").unwrap(),
            original
        );
        fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn native_start_stop_and_get_wire_types_roundtrip_exact_contract() {
        use zbus::zvariant::{LE, Type, serialized::Context, to_bytes};
        type Properties = Vec<(String, OwnedValue)>;
        type Start = (String, String, Properties, Vec<(String, Properties)>);
        let (_, intent) = crate::domain_runtime::tests::fixture::coordinator();
        let selection = ServiceSelection {
            environment_cleaner: "/owned/env".into(),
            bootstrap: "/owned/bootstrap".into(),
            controller_dir: "/owned/control".into(),
            runtime_dir: "/owned/runtime".into(),
            delegated_root: "/owned/cgroup".into(),
            executable_bytes: 1024,
            nonce: "public-nonce".into(),
            limits: ServiceLimits {
                wall_ms: 1000,
                cpu_quota_us: 100000,
                cpu_period_us: 100000,
                memory_bytes: 1024,
                swap_bytes: 0,
                tasks: 2,
            },
        };
        let properties = creation_properties(&selection, &intent).unwrap();
        let body: Start = (unit(&intent), "fail".into(), properties, vec![]);
        assert_eq!(Start::SIGNATURE.to_string(), "(ssa(sv)a(sa(sv)))");
        let bytes = to_bytes(Context::new_dbus(LE, 0), &body).unwrap();
        let (decoded, used): (Start, usize) = bytes.deserialize().unwrap();
        assert_eq!(used, bytes.len());
        assert_eq!(decoded, body);
        let command = decoded
            .2
            .iter()
            .find(|(k, _)| k == "ExecStart")
            .unwrap()
            .1
            .try_clone()
            .unwrap();
        let command: Vec<(String, Vec<String>, bool)> = command.try_into().unwrap();
        assert_eq!(
            command,
            vec![(
                "/owned/env".into(),
                vec![
                    "/owned/env".into(),
                    "-i".into(),
                    "/owned/bootstrap".into(),
                    format!("/owned/control/{}.sock", intent.request.as_str()),
                    intent.request.as_str().into(),
                    "1000".into()
                ],
                false
            )]
        );
        assert!(!command[0].1.iter().any(|a| a == "public-nonce"));
        let get = (unit(&intent),);
        let bytes = to_bytes(Context::new_dbus(LE, 0), &get).unwrap();
        assert_eq!(bytes.deserialize::<(String,)>().unwrap().0, get);
        let stop = (unit(&intent), "fail".to_string());
        let bytes = to_bytes(Context::new_dbus(LE, 0), &stop).unwrap();
        assert_eq!(bytes.deserialize::<(String, String)>().unwrap().0, stop);
        let object = OwnedObjectPath::try_from("/org/freedesktop/systemd1/job/1").unwrap();
        let bytes = to_bytes(Context::new_dbus(LE, 0), &object).unwrap();
        assert_eq!(bytes.deserialize::<OwnedObjectPath>().unwrap().0, object);
    }
    #[test]
    fn direct_member_parser_rejects_ambiguity_and_bounds_without_host_effects() {
        assert_eq!(member_ids("7\n3\n").unwrap(), vec![3, 7]);
        assert!(member_ids("").unwrap().is_empty());
        for invalid in ["0", "7 7", "not-a-pid", "4294967296", "1 2 3 4 5 6 7 8 9"] {
            assert!(member_ids(invalid).is_err());
        }
    }
    #[test]
    fn expired_call_and_duplicate_kernel_counters_are_refused() {
        let result = bounded(
            async { Ok::<_, zbus::Error>(1) },
            DomainPhaseDiagnostic::Identity,
            Instant::now() - Duration::from_millis(1),
        );
        assert_eq!(result.unwrap_err().cause, DomainCause::Deadline);
        assert_eq!(key_number("populated 0\npopulated 1\n", "populated"), None);
        assert_eq!(key_number("populated 0\nfrozen 0\n", "populated"), Some(0));
    }
    #[test]
    fn exact_cleanup_stop_allows_absent_parent_but_not_reused_incarnation() {
        use crate::domain_runtime::tests::fixture;
        let (_, intent) = fixture::coordinator();
        let identity = fixture::identity(&intent);
        let observed = ManagedObservation {
            identity: identity.clone(),
            main_pid: 0,
            start_ticks: 0,
            populated: true,
            direct_members: vec![DomainProcessObservation {
                pid: 42,
                start_ticks: 7,
            }],
            controls: fixture::binding(&intent).controls,
            cpu_micros: None,
            memory_peak: None,
            restart_prevented: true,
        };
        let mut requests = 0;
        checked_stop_request(&observed, &identity, || {
            requests += 1;
            Ok(())
        })
        .unwrap();
        assert_eq!(requests, 1);
        for case in 0..5 {
            let mut wrong = identity.clone();
            match case {
                0 => wrong.invocation = "different-invocation".into(),
                1 => wrong.manager = "different-manager".into(),
                2 => wrong.cgroup_inode += 1,
                3 => wrong.boot = "different-boot".into(),
                _ => wrong.request = OperationId::new("different-request").unwrap(),
            }
            assert!(
                checked_stop_request(&observed, &wrong, || {
                    requests += 1;
                    Ok(())
                })
                .is_err()
            );
            assert_eq!(requests, 1);
        }
        let lost: Result<(), DomainDiagnostic> = checked_stop_request(&observed, &identity, || {
            requests += 1;
            Err(issue(
                DomainPhaseDiagnostic::Closure,
                DomainCause::ManagerRefusal,
            ))
        });
        assert_eq!(lost.unwrap_err().cause, DomainCause::ManagerRefusal);
        assert_eq!(requests, 2); // No retry, absence inference or cleanup claim.
    }
}
