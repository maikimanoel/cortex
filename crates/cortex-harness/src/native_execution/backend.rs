//! Existing canonical domain adapter for a strict trusted executable.
//! No host invocation is permitted without an explicitly authorized delegation.
use super::{
    ExecutionError, ExecutionPlan, FileSpec, PinnedFile, artifacts, bootstrap,
    cgroup::{Delegation, DelegationAuthorization, OwnedGroup},
    transport,
};
use crate::{
    domain_manager::*,
    domain_runtime::DomainManager,
    native_linux::{NativeIsolation, StableProcess},
};
use cortex_core::program::{domain::*, operational::EffectOutcome, supervision::ProcessCompletion};
use cortex_evidence::program::CoordinationKernel;
use serde_json::json;
use std::{
    fs,
    os::unix::{fs::MetadataExt, net::UnixStream},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::atomic::Ordering,
    time::{Duration, Instant},
};

/// Process/stream/artifact observations remain separate from durable completion.
pub struct ExecutionObservation {
    /// Direct process and bounded stream evidence.
    pub streams: transport::CapturedStreams,
    /// Explicit expected artifacts, only collected after actual recursive emptiness.
    pub artifacts: Vec<artifacts::CapturedArtifact>,
    /// Kernel recursive population, absent if observation failed.
    pub domain_populated: Option<bool>,
    /// Present only for explicitly selected JSON-lines and exact canonical context.
    pub structured: Option<crate::structured_output::BoundStructuredObservation>,
}
/// Owned strict adapter. It never uses service-name termination or adopts a
/// recovered PID. Drop retains canonical uncertainty and performs no settlement.
pub struct NativeExecutionDomain {
    plan: ExecutionPlan,
    bootstrap: FileSpec,
    _bootstrap_pin: PinnedFile,
    runtime: PathBuf,
    runtime_identity: (u64, u64),
    isolation: NativeIsolation,
    delegation: Delegation,
    group: Option<OwnedGroup>,
    child: Option<Child>,
    scratch: Option<fs::File>,
    identity: Option<DomainIdentity>,
    observation: Option<ExecutionObservation>,
}
fn diagnostic(e: ExecutionError) -> DomainDiagnostic {
    issue(
        DomainPhaseDiagnostic::Identity,
        match e {
            ExecutionError::Authority => DomainCause::Authority,
            ExecutionError::Io(kind) => DomainCause::Io(kind),
            _ => DomainCause::Correspondence,
        },
    )
}
impl NativeExecutionDomain {
    /// Pin controller inputs without executing anything. Missing authorization
    /// refuses before opening or creating any cgroup. Root/runtime must be fresh,
    /// empty owner-only directories; no host scope is discovered automatically.
    pub fn inspect(
        plan: ExecutionPlan,
        bootstrap: FileSpec,
        runtime: &Path,
        isolation: NativeIsolation,
        authorization: Option<DelegationAuthorization>,
    ) -> Result<Self, ExecutionError> {
        let delegation = Delegation::inspect(authorization)?;
        let m = fs::symlink_metadata(runtime)?;
        if fs::canonicalize(runtime)? != runtime
            || !m.is_dir()
            || m.mode() & 0o777 != 0o700
            || m.uid() != rustix::process::getuid().as_raw()
            || fs::read_dir(runtime)?.next().is_some()
            || runtime.as_os_str().len() > 80
        {
            return Err(ExecutionError::Policy);
        }
        isolation.check()?;
        if isolation.scratch_limits() != (plan.spec.scratch_bytes, plan.spec.scratch_inodes)
            || plan.spec.limits.wall_ms > 15000
        {
            return Err(ExecutionError::Policy);
        }
        let pin = PinnedFile::inspect(&bootstrap)?;
        Ok(Self {
            plan,
            bootstrap,
            _bootstrap_pin: pin,
            runtime: runtime.into(),
            runtime_identity: (m.dev(), m.ino()),
            isolation,
            delegation,
            group: None,
            child: None,
            scratch: None,
            identity: None,
            observation: None,
        })
    }
    pub(crate) fn wall_ms(&self) -> u64 {
        self.plan.spec.limits.wall_ms
    }
    pub(crate) fn digest(&self) -> &str {
        self.plan.digest()
    }
    pub(crate) fn endpoint(&self) -> PathBuf {
        self.runtime.join("control")
    }
    pub(crate) fn take_observation(&mut self) -> Option<ExecutionObservation> {
        self.observation.take()
    }
    fn context(&self) -> Result<(), ExecutionError> {
        let m = fs::symlink_metadata(&self.runtime)?;
        if (m.dev(), m.ino()) != self.runtime_identity
            || m.mode() & 0o777 != 0o700
            || !m.is_dir()
            || fs::canonicalize(&self.runtime)? != self.runtime
        {
            return Err(ExecutionError::Identity);
        }
        self.isolation.check()?;
        Ok(())
    }
}
impl DomainManager for NativeExecutionDomain {
    fn check_context(
        &self,
        k: &CoordinationKernel,
        i: &DomainIntent,
        at: u64,
    ) -> Result<(), DomainDiagnostic> {
        self.context().map_err(diagnostic)?;
        self.plan.check(k, i, at).map_err(diagnostic)?;
        canonical_manager_guarantees().map_err(diagnostic)
    }
    fn is_nonce(&self) -> bool {
        false
    }
    fn verify_ready(&self, hello: &serde_json::Value) -> bool {
        hello.as_array().is_some_and(|a| a.len() == 7) && hello[6] == self.plan.digest()
    }
    fn pin_process(
        &self,
        a: &ManagedObservation,
    ) -> Result<Option<StableProcess>, DomainDiagnostic> {
        StableProcess::acquire(a.main_pid, a.start_ticks)
            .map(Some)
            .map_err(|e| diagnostic(e.into()))
    }
    fn corresponds(&self, i: &DomainIntent) -> Result<(), DomainDiagnostic> {
        if i.configuration != self.plan.configuration().map_err(diagnostic)? {
            return Err(diagnostic(ExecutionError::Identity));
        }
        self.context().map_err(diagnostic)
    }
    fn create(&mut self, i: &DomainIntent, end: Instant) -> Result<(), DomainDiagnostic> {
        if self.child.is_some() || self.group.is_some() || Instant::now() >= end {
            return Err(diagnostic(ExecutionError::Identity));
        }
        let group = self
            .delegation
            .create(i.request.as_str())
            .map_err(diagnostic)?;
        self.group = Some(group);
        self.group
            .as_mut()
            .unwrap()
            .configure(&self.plan.spec.controls)
            .map_err(diagnostic)?;
        // Same-user executable pathname replacement remains a cooperating-host
        // trust boundary. Recheck approved bytes immediately before spawn; there
        // is no memfd/noexec workaround or unrestricted alternate binary.
        let _current = PinnedFile::inspect(&self.bootstrap).map_err(diagnostic)?;
        let child = Command::new(&self.bootstrap.source)
            .args([
                self.endpoint().to_string_lossy().into_owned(),
                i.request.as_str().into(),
                self.plan.digest().into(),
                self.isolation.root_path().to_string_lossy().into_owned(),
                self.isolation.digest(),
                end.saturating_duration_since(Instant::now())
                    .as_millis()
                    .to_string(),
            ])
            .env_clear()
            .current_dir(&self.runtime)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| diagnostic(e.into()))?;
        self.child = Some(child);
        self.group
            .as_mut()
            .unwrap()
            .attach(self.child.as_mut().unwrap())
            .map_err(diagnostic)?;
        let (device, inode) = self.group.as_ref().unwrap().identity();
        self.identity = Some(DomainIdentity {
            pool: i.pool.clone(),
            request: i.request.clone(),
            manager: "native-cgroup-v1".into(),
            invocation: format!("group-{device}-{inode}"),
            control_group: self
                .group
                .as_ref()
                .unwrap()
                .path()
                .to_string_lossy()
                .into_owned(),
            cgroup_device: device,
            cgroup_inode: inode,
            boot: i.pool.boot.clone(),
        });
        Ok(())
    }
    fn setup(&mut self, channel: &mut UnixStream, end: Instant) -> Result<(), DomainDiagnostic> {
        let peer =
            rustix::net::sockopt::socket_peercred(&*channel).map_err(|e| diagnostic(e.into()))?;
        let child = self
            .child
            .as_mut()
            .ok_or_else(|| diagnostic(ExecutionError::Identity))?;
        if peer.pid.as_raw_pid() as u32 != child.id()
            || child
                .try_wait()
                .map_err(|e| diagnostic(e.into()))?
                .is_some()
        {
            return Err(diagnostic(ExecutionError::Identity));
        }
        self.group
            .as_ref()
            .unwrap()
            .check_controls()
            .map_err(diagnostic)?;
        crate::domain_bootstrap::frame_write(
            channel,
            &json!([
                1,
                "setup",
                self.plan.digest(),
                self.plan.spec.scratch_bytes,
                self.plan.spec.scratch_inodes,
                self.plan
                    .spec
                    .files
                    .iter()
                    .map(bootstrap::file_value)
                    .collect::<Vec<_>>(),
                self.plan.spec.arguments,
                self.plan.spec.environment,
                self.plan.spec.working_directory,
                [
                    self.group
                        .as_ref()
                        .unwrap()
                        .path()
                        .to_string_lossy()
                        .into_owned(),
                    self.group.as_ref().unwrap().identity().0.to_string(),
                    self.group.as_ref().unwrap().identity().1.to_string(),
                    self.identity.as_ref().unwrap().boot.clone(),
                    self.plan.spec.controls.cpu_quota_us.to_string(),
                    self.plan.spec.controls.cpu_period_us.to_string(),
                    self.plan.spec.controls.memory_bytes.to_string(),
                    self.plan.spec.controls.swap_bytes.to_string(),
                    self.plan.spec.controls.tasks_max.to_string()
                ]
            ]),
            end,
        )
        .map_err(|e| diagnostic(e.into()))
    }
    fn observe(
        &mut self,
        _: &DomainIntent,
        _: Instant,
    ) -> Result<ManagedObservation, DomainDiagnostic> {
        canonical_manager_guarantees().map_err(diagnostic)?;
        let group = self
            .group
            .as_ref()
            .ok_or_else(|| diagnostic(ExecutionError::Identity))?;
        let c = group.check_controls().map_err(diagnostic)?;
        let o = group.observe().map_err(diagnostic)?;
        let child = self
            .child
            .as_ref()
            .ok_or_else(|| diagnostic(ExecutionError::Identity))?;
        let pid = child.id();
        let ticks = crate::domain_bootstrap::ticks(pid).map_err(|e| diagnostic(e.into()))?;
        let stable = StableProcess::acquire(pid, ticks).map_err(|e| diagnostic(e.into()))?;
        stable.require_live().map_err(|e| diagnostic(e.into()))?;
        let scratch = fs::File::open(format!("/proc/{pid}/root/scratch"))
            .map_err(|e| diagnostic(e.into()))?;
        if rustix::fs::fstatfs(&scratch)
            .map_err(|e| diagnostic(e.into()))?
            .f_type as u64
            != 0x01021994
        {
            return Err(diagnostic(ExecutionError::Identity));
        }
        stable.require_live().map_err(|e| diagnostic(e.into()))?;
        if let Some(previous) = &self.scratch {
            let a = previous.metadata().map_err(|e| diagnostic(e.into()))?;
            let b = scratch.metadata().map_err(|e| diagnostic(e.into()))?;
            if (a.dev(), a.ino()) != (b.dev(), b.ino()) {
                return Err(diagnostic(ExecutionError::Identity));
            }
        } else {
            self.scratch = Some(scratch);
        }
        let members = o
            .members
            .iter()
            .map(|pid| {
                Ok(DomainProcessObservation {
                    pid: *pid,
                    start_ticks: crate::domain_bootstrap::ticks(*pid)
                        .map_err(|e| diagnostic(e.into()))?,
                })
            })
            .collect::<Result<Vec<_>, DomainDiagnostic>>()?;
        Ok(ManagedObservation {
            identity: self
                .identity
                .clone()
                .ok_or_else(|| diagnostic(ExecutionError::Identity))?,
            main_pid: pid,
            start_ticks: ticks,
            populated: o.populated,
            direct_members: members,
            controls: DomainControlReadback {
                cpu_quota_us: c.cpu_quota_us,
                cpu_period_us: c.cpu_period_us,
                memory_bytes: c.memory_bytes,
                swap_bytes: c.swap_bytes,
                tasks: c.tasks_max,
                runtime_max_micros: self.plan.spec.limits.wall_ms * 1000,
                timeout_stop_micros: self.plan.spec.limits.cleanup_ms * 1000,
                restart_prevented: true,
                kill_control_group: true,
                no_new_privileges: true,
                send_sigkill: true,
                remain_after_exit: false,
            },
            cpu_micros: o.cpu_micros,
            memory_peak: o.memory_peak,
            restart_prevented: true,
        })
    }
    fn stop(
        &mut self,
        _: &DomainIntent,
        identity: &DomainIdentity,
        end: Instant,
    ) -> Result<(), DomainDiagnostic> {
        if self.identity.as_ref() != Some(identity) {
            return Err(diagnostic(ExecutionError::Identity));
        }
        let group = self
            .group
            .as_ref()
            .ok_or_else(|| diagnostic(ExecutionError::Identity))?;
        group.request_termination().map_err(diagnostic)?;
        while Instant::now() < end {
            if !group.observe().map_err(diagnostic)?.populated {
                if let Some(child) = self.child.as_mut() {
                    let _ = child.try_wait();
                }
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        Err(issue(DomainPhaseDiagnostic::Closure, DomainCause::Deadline))
    }
    fn finish(
        &mut self,
        _: &mut UnixStream,
        context: crate::domain_runtime::DomainFinish<'_>,
        authority: &mut dyn FnMut() -> Result<(), DomainDiagnostic>,
    ) -> Result<EffectOutcome, DomainDiagnostic> {
        let end = context.end;
        let cancel = context.cancel;
        use cortex_core::program::supervision::InterruptionReason;
        let child = self
            .child
            .as_mut()
            .ok_or_else(|| diagnostic(ExecutionError::Identity))?;
        let mut check = || {
            if cancel.load(Ordering::SeqCst) {
                return Err(InterruptionReason::Cancelled);
            }
            authority().map_err(|_| InterruptionReason::AuthorityUnknown)
        };
        let (streams, structured) = if self.plan.uses_json_lines() {
            let captured = transport::collect_structured(
                child,
                self.plan.input(),
                &self.plan.spec.limits,
                context.start,
                end,
                &mut check,
            )
            .map_err(diagnostic)?;
            let bound = crate::structured_output::BoundStructuredObservation::new(
                context.intent,
                context.binding,
                context.release,
                captured.structured,
            )
            .map_err(|_| diagnostic(ExecutionError::Identity))?;
            (captured.streams, Some(bound))
        } else {
            (
                transport::collect(
                    child,
                    self.plan.input(),
                    &self.plan.spec.limits,
                    end,
                    &mut check,
                )
                .map_err(diagnostic)?,
                None,
            )
        };
        let group = self
            .group
            .as_ref()
            .ok_or_else(|| diagnostic(ExecutionError::Identity))?;
        let populated = group.observe().ok().map(|o| o.populated);
        let captured = if populated == Some(false) {
            match self.scratch.take() {
                Some(scratch) => artifacts::capture(
                    &scratch,
                    &self.plan.spec.artifacts,
                    self.plan.spec.scratch_bytes,
                )
                .map_err(diagnostic)?,
                None => return Err(diagnostic(ExecutionError::Identity)),
            }
        } else {
            Vec::new()
        };
        let succeeded = matches!(
            streams.completion,
            ProcessCompletion::Exited {
                code: Some(0),
                signal: None
            }
        ) && streams.input_complete
            && streams.stdout_complete
            && streams.stderr_complete
            && populated == Some(false);
        self.observation = Some(ExecutionObservation {
            streams,
            artifacts: captured,
            domain_populated: populated,
            structured,
        });
        if succeeded {
            Ok(EffectOutcome::Succeeded)
        } else {
            Err(diagnostic(ExecutionError::Identity))
        }
    }
}

/// Current adapter lacks an independent manager lifetime timer and retained
/// manager unit. Controller timers and persistent cgroup directories cannot
/// supply the existing canonical manager readback contract. Refusal precedes
/// CreateRequested, cgroup mutation and bootstrap launch.
pub fn canonical_manager_guarantees() -> Result<(), ExecutionError> {
    Err(ExecutionError::ManagerGuarantees)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_manager_guarantees_refuse_instead_of_fabricating_binding() {
        assert_eq!(
            canonical_manager_guarantees(),
            Err(ExecutionError::ManagerGuarantees)
        );
    }
}
